//! Read-only AcroForm field extraction (ISO 32000-1 §12.7).
//!
//! The field hierarchy under `/AcroForm /Fields` is walked into an arena,
//! inheritable attributes are resolved once per logical field, every page's
//! `/Annots` is scanned for `/Subtype /Widget` annotations, and the two are
//! joined by object identity. One `PageFormField` is emitted per placed
//! widget; logical fields without a placement are reported on `FormInfo`;
//! widgets no field claims are emitted as `Unknown`.
//!
//! Nothing here writes: no `/V` updates, no appearance streams, no flattening.

use crate::types::{
  ChoiceOption, FieldCommon, FieldKind, FormInfo, PageFormField, UnplacedFormField,
};
use lopdf::{Dictionary, Document, Object, ObjectId};
use std::collections::{HashMap, HashSet};

/// `/Kids` nesting deeper than this is not visited; `FormInfo.truncated` reports it.
const MAX_DEPTH: usize = 100;

// ── Small helpers ─────────────────────────────────────────────────

/// Same format as `PageImage.objectId`: `"N G obj"`.
pub(crate) fn object_id_string(id: ObjectId) -> String {
  format!("{} {} obj", id.0, id.1)
}

/// `/Name` bytes are not PDF text strings: try UTF-8, else map each byte to
/// the Latin-1 code point of the same value.
fn decode_name_bytes(bytes: &[u8]) -> String {
  match std::str::from_utf8(bytes) {
    Ok(s) => s.to_string(),
    Err(_) => bytes.iter().map(|&b| b as char).collect(),
  }
}

/// Resolve one object through the document. A dangling reference is reported
/// as a warning and treated as absent; non-references pass through untouched.
fn deref<'a>(
  doc: &'a Document,
  obj: &'a Object,
  warnings: &mut Vec<String>,
  what: &str,
) -> Option<&'a Object> {
  match obj {
    Object::Reference(id) => match doc.get_object(*id) {
      Ok(o) => Some(o),
      Err(_) => {
        warnings.push(format!("{what}: unresolved reference {} {} R", id.0, id.1));
        None
      }
    },
    other => Some(other),
  }
}

/// Decode a value-like object to text: text strings (UTF-16BE BOM, UTF-8 BOM,
/// PDFDocEncoding), names, numbers, or the first element of an array.
fn decode_text_object(obj: &Object, doc: &Document, warnings: &mut Vec<String>) -> Option<String> {
  let obj = deref(doc, obj, warnings, "value")?;
  match obj {
    Object::String(bytes, _) => Some(
      lopdf::decode_text_string(obj).unwrap_or_else(|_| String::from_utf8_lossy(bytes).to_string()),
    ),
    Object::Name(n) => Some(decode_name_bytes(n)),
    Object::Integer(i) => Some(i.to_string()),
    Object::Real(r) => Some(r.to_string()),
    Object::Array(arr) => arr
      .first()
      .and_then(|o| decode_text_object(o, doc, warnings)),
    _ => None,
  }
}

/// All decoded string entries of a (possibly indirect) array value.
fn decode_text_array(
  obj: &Object,
  doc: &Document,
  warnings: &mut Vec<String>,
) -> Option<Vec<String>> {
  match deref(doc, obj, warnings, "value")? {
    Object::Array(arr) => Some(
      arr
        .iter()
        .filter_map(|o| decode_text_object(o, doc, warnings))
        .collect(),
    ),
    _ => None,
  }
}

/// Every numeric entry of `/Rect` in file order; non-numeric entries are
/// skipped (same convention as `PageAnnotation.rect`).
fn parse_rect(obj: &Object) -> Vec<f64> {
  match obj {
    Object::Array(arr) => arr
      .iter()
      .filter_map(|o| match o {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(*r as f64),
        _ => None,
      })
      .collect(),
    _ => Vec::new(),
  }
}

/// `/Opt` entries: a plain string (export = display) or an `[export display]` pair.
fn parse_options(doc: &Document, obj: &Object, warnings: &mut Vec<String>) -> Vec<ChoiceOption> {
  let Some(Object::Array(entries)) = deref(doc, obj, warnings, "/Opt") else {
    return Vec::new();
  };
  entries
    .iter()
    .filter_map(|entry| match deref(doc, entry, warnings, "/Opt entry")? {
      Object::Array(pair) => {
        let export = pair
          .first()
          .and_then(|o| decode_text_object(o, doc, warnings))?;
        let display = pair
          .get(1)
          .and_then(|o| decode_text_object(o, doc, warnings))
          .unwrap_or_else(|| export.clone());
        Some(ChoiceOption {
          export_value: export,
          display_value: display,
        })
      }
      other => {
        let text = decode_text_object(other, doc, warnings)?;
        Some(ChoiceOption {
          export_value: text.clone(),
          display_value: text,
        })
      }
    })
    .collect()
}

fn dict_int(
  doc: &Document,
  dict: &Dictionary,
  key: &[u8],
  warnings: &mut Vec<String>,
) -> Option<i64> {
  let obj = dict.get(key).ok()?;
  match deref(
    doc,
    obj,
    warnings,
    &format!("/{}", String::from_utf8_lossy(key)),
  )? {
    Object::Integer(i) => Some(*i),
    Object::Real(r) => Some(*r as i64),
    _ => None,
  }
}

fn dict_name(
  doc: &Document,
  dict: &Dictionary,
  key: &[u8],
  warnings: &mut Vec<String>,
) -> Option<String> {
  let obj = dict.get(key).ok()?;
  match deref(
    doc,
    obj,
    warnings,
    &format!("/{}", String::from_utf8_lossy(key)),
  )? {
    Object::Name(n) => Some(decode_name_bytes(n)),
    _ => None,
  }
}

fn dict_text(
  doc: &Document,
  dict: &Dictionary,
  key: &[u8],
  warnings: &mut Vec<String>,
) -> Option<String> {
  dict
    .get(key)
    .ok()
    .and_then(|o| decode_text_object(o, doc, warnings))
}

// ── Field flags and types (ISO 32000-1 Tables 221, 226, 228, 230) ──

/// `/Ff` bit set. Bit numbers are 1-based as in the spec: bit N is `1 << (N - 1)`.
#[derive(Clone, Copy, Default, Debug)]
struct FieldFlags(u32);

impl FieldFlags {
  fn bit(self, n: u32) -> bool {
    self.0 & (1u32 << (n - 1)) != 0
  }
  fn read_only(self) -> bool {
    self.bit(1)
  }
  fn required(self) -> bool {
    self.bit(2)
  }
  fn no_export(self) -> bool {
    self.bit(3)
  }
  fn multiline(self) -> bool {
    self.bit(13)
  }
  fn password(self) -> bool {
    self.bit(14)
  }
  fn radio(self) -> bool {
    self.bit(16)
  }
  fn push_button(self) -> bool {
    self.bit(17)
  }
  fn combo(self) -> bool {
    self.bit(18)
  }
  fn edit(self) -> bool {
    self.bit(19)
  }
  fn multi_select(self) -> bool {
    self.bit(22)
  }
  fn comb(self) -> bool {
    self.bit(25)
  }
  fn rich_text(self) -> bool {
    self.bit(26)
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FieldType {
  Text,
  Button,
  Choice,
  Signature,
}

impl FieldType {
  fn from_name(name: &[u8]) -> Option<Self> {
    match name {
      b"Tx" => Some(FieldType::Text),
      b"Btn" => Some(FieldType::Button),
      b"Ch" => Some(FieldType::Choice),
      b"Sig" => Some(FieldType::Signature),
      _ => None,
    }
  }
}

// ── Field tree ────────────────────────────────────────────────────

/// One dictionary from the `/Fields` hierarchy: a field, a widget, or both.
struct FieldNode {
  /// Set when the dictionary was reached through a reference.
  object_id: Option<ObjectId>,
  /// Position in the parent's `/Kids` (or the root `/Fields`) when written inline.
  inline_index: Option<usize>,
  is_root: bool,
  partial_name: String,
  field_type: Option<FieldType>,
  flags: Option<u32>,
  value: Option<Object>,
  default_value: Option<Object>,
  max_len: Option<u32>,
  options: Option<Vec<ChoiceOption>>,
  tooltip: Option<String>,
  mapping_name: Option<String>,
  has_actions: bool,
  parent: Option<usize>,
  children: Vec<usize>,
  dict: Dictionary,
}

impl FieldNode {
  fn from_dict(
    doc: &Document,
    dict: Dictionary,
    object_id: Option<ObjectId>,
    inline_index: Option<usize>,
    is_root: bool,
    parent: Option<usize>,
    warnings: &mut Vec<String>,
  ) -> Self {
    let partial_name = dict_text(doc, &dict, b"T", warnings).unwrap_or_default();
    let field_type = dict
      .get(b"FT")
      .ok()
      .and_then(|o| deref(doc, o, warnings, "/FT"))
      .and_then(|o| o.as_name().ok())
      .and_then(FieldType::from_name);
    let flags = dict_int(doc, &dict, b"Ff", warnings).map(|v| v as u32);
    let max_len = dict_int(doc, &dict, b"MaxLen", warnings).and_then(|v| u32::try_from(v).ok());
    let options = dict
      .get(b"Opt")
      .ok()
      .map(|o| parse_options(doc, o, warnings));
    let tooltip = dict_text(doc, &dict, b"TU", warnings);
    let mapping_name = dict_text(doc, &dict, b"TM", warnings);
    let has_actions = dict.has(b"A") || dict.has(b"AA");
    FieldNode {
      object_id,
      inline_index,
      is_root,
      partial_name,
      field_type,
      flags,
      value: dict.get(b"V").ok().cloned(),
      default_value: dict.get(b"DV").ok().cloned(),
      max_len,
      options,
      tooltip,
      mapping_name,
      has_actions,
      parent,
      children: Vec::new(),
      dict,
    }
  }
}

struct FieldTree {
  nodes: Vec<FieldNode>,
  warnings: Vec<String>,
  truncated: bool,
}

impl FieldTree {
  fn walk(doc: &Document, fields: &[Object]) -> FieldTree {
    let mut tree = FieldTree {
      nodes: Vec::new(),
      warnings: Vec::new(),
      truncated: false,
    };
    let mut visited: HashSet<ObjectId> = HashSet::new();
    for (i, entry) in fields.iter().enumerate() {
      tree.visit(doc, entry, None, i, 0, &mut visited);
    }
    tree
  }

  fn visit(
    &mut self,
    doc: &Document,
    entry: &Object,
    parent: Option<usize>,
    index_in_parent: usize,
    depth: usize,
    visited: &mut HashSet<ObjectId>,
  ) {
    if depth >= MAX_DEPTH {
      if !self.truncated {
        let under = parent.map(|p| self.full_name(p)).unwrap_or_default();
        self.warnings.push(format!(
          "field tree depth cap ({MAX_DEPTH}) reached under \"{under}\"; deeper fields were not read"
        ));
      }
      self.truncated = true;
      return;
    }
    let (object_id, dict) = match entry {
      Object::Reference(id) => {
        if !visited.insert(*id) {
          // Already part of the tree: a /Kids cycle or a shared node.
          return;
        }
        match doc.get_dictionary(*id) {
          Ok(d) => (Some(*id), d.clone()),
          Err(_) => {
            self
              .warnings
              .push(format!("/Kids: unresolved reference {} {} R", id.0, id.1));
            return;
          }
        }
      }
      Object::Dictionary(d) => (None, d.clone()),
      other => {
        self.warnings.push(format!(
          "/Kids: entry {index_in_parent} is not a dictionary ({})",
          object_kind(other)
        ));
        return;
      }
    };

    let inline_index = if object_id.is_none() {
      Some(index_in_parent)
    } else {
      None
    };
    let node = FieldNode::from_dict(
      doc,
      dict,
      object_id,
      inline_index,
      parent.is_none(),
      parent,
      &mut self.warnings,
    );
    let idx = self.nodes.len();
    self.nodes.push(node);
    if let Some(p) = parent {
      self.nodes[p].children.push(idx);
    }

    if let Ok(kids_obj) = self.nodes[idx].dict.get(b"Kids").cloned() {
      match deref(doc, &kids_obj, &mut self.warnings, "/Kids") {
        Some(Object::Array(kids)) => {
          let kids = kids.clone();
          for (k, kid) in kids.iter().enumerate() {
            self.visit(doc, kid, Some(idx), k, depth + 1, visited);
          }
        }
        Some(_) => self.warnings.push(format!(
          "field \"{}\": /Kids is not an array",
          self.full_name(idx)
        )),
        None => {}
      }
    }
  }

  fn ancestors_and_self(&self, i: usize) -> impl Iterator<Item = &FieldNode> {
    let mut chain = Vec::new();
    let mut cur = Some(i);
    while let Some(c) = cur {
      chain.push(&self.nodes[c]);
      cur = self.nodes[c].parent;
    }
    chain.into_iter()
  }

  fn effective_type(&self, i: usize) -> Option<FieldType> {
    self.ancestors_and_self(i).find_map(|n| n.field_type)
  }

  fn effective_flags(&self, i: usize) -> FieldFlags {
    FieldFlags(
      self
        .ancestors_and_self(i)
        .find_map(|n| n.flags)
        .unwrap_or(0),
    )
  }

  fn effective_value(&self, i: usize) -> Option<&Object> {
    self.ancestors_and_self(i).find_map(|n| n.value.as_ref())
  }

  fn effective_default(&self, i: usize) -> Option<&Object> {
    self
      .ancestors_and_self(i)
      .find_map(|n| n.default_value.as_ref())
  }

  fn effective_max_len(&self, i: usize) -> Option<u32> {
    self.ancestors_and_self(i).find_map(|n| n.max_len)
  }

  fn effective_options(&self, i: usize) -> Option<&[ChoiceOption]> {
    self
      .ancestors_and_self(i)
      .find_map(|n| n.options.as_deref())
  }

  fn effective_tooltip(&self, i: usize) -> Option<&str> {
    self
      .ancestors_and_self(i)
      .find_map(|n| n.tooltip.as_deref())
  }

  fn effective_mapping_name(&self, i: usize) -> Option<&str> {
    self
      .ancestors_and_self(i)
      .find_map(|n| n.mapping_name.as_deref())
  }

  /// Non-empty partial names from the root down, joined with `.`.
  fn full_name(&self, i: usize) -> String {
    let mut parts: Vec<&str> = self
      .ancestors_and_self(i)
      .map(|n| n.partial_name.as_str())
      .filter(|s| !s.is_empty())
      .collect();
    parts.reverse();
    parts.join(".")
  }

  /// A logical field has an effective type, no named children, and is either
  /// named itself or sits directly in `/Fields`. An unnamed node with a parent
  /// is a widget of that parent, never a field of its own.
  fn is_logical(&self, i: usize) -> bool {
    let node = &self.nodes[i];
    self.effective_type(i).is_some()
      && !node
        .children
        .iter()
        .any(|&c| !self.nodes[c].partial_name.is_empty())
      && (!node.partial_name.is_empty() || node.is_root)
  }

  /// Identity for joining placements: the object id when the node was
  /// referenced, else a synthetic id anchored on the nearest referenced ancestor.
  fn field_object_id(&self, i: usize) -> String {
    let node = &self.nodes[i];
    match (node.object_id, node.inline_index, node.parent) {
      (Some(id), _, _) => object_id_string(id),
      (None, Some(k), Some(p)) => format!("{}/kid {k}", self.field_object_id(p)),
      (None, Some(k), None) => format!("/Fields {k}"),
      (None, None, _) => String::from("inline"),
    }
  }
}

fn object_kind(obj: &Object) -> &'static str {
  match obj {
    Object::Null => "null",
    Object::Boolean(_) => "boolean",
    Object::Integer(_) => "integer",
    Object::Real(_) => "real",
    Object::Name(_) => "name",
    Object::String(..) => "string",
    Object::Array(_) => "array",
    Object::Dictionary(_) => "dictionary",
    Object::Stream(_) => "stream",
    Object::Reference(_) => "reference",
  }
}

// ── Widgets on pages ──────────────────────────────────────────────

struct PlacedWidget {
  object_id: String,
  /// `None` for a dictionary written inline in `/Annots`.
  key: Option<ObjectId>,
  page: u32,
  rect: Vec<f64>,
  dict: Dictionary,
}

/// Every `/Subtype /Widget` annotation on every page, whether `/Annots` is a
/// direct array or a reference to one. Inline dictionaries get the synthetic
/// id `"page P annot I"`.
fn scan_widgets(doc: &Document, warnings: &mut Vec<String>) -> Vec<PlacedWidget> {
  let mut widgets = Vec::new();
  for (page_num, page_id) in doc.get_pages() {
    let Ok(page) = doc.get_dictionary(page_id) else {
      continue;
    };
    let Ok(annots_obj) = page.get(b"Annots") else {
      continue;
    };
    let annots = match deref(
      doc,
      annots_obj,
      warnings,
      &format!("page {page_num}: /Annots"),
    ) {
      Some(Object::Array(a)) => a,
      Some(_) => {
        warnings.push(format!("page {page_num}: /Annots is not an array"));
        continue;
      }
      None => continue,
    };
    for (index, entry) in annots.iter().enumerate() {
      let (key, dict) = match entry {
        Object::Reference(id) => match doc.get_dictionary(*id) {
          Ok(d) => (Some(*id), d),
          Err(_) => {
            warnings.push(format!(
              "page {page_num}: /Annots entry {index} is an unresolved reference {} {} R",
              id.0, id.1
            ));
            continue;
          }
        },
        Object::Dictionary(d) => (None, d),
        _ => continue,
      };
      let is_widget = dict
        .get(b"Subtype")
        .ok()
        .and_then(|o| o.as_name().ok())
        .is_some_and(|n| n == b"Widget");
      if !is_widget {
        continue;
      }
      let rect = dict
        .get(b"Rect")
        .ok()
        .and_then(|o| deref(doc, o, warnings, "/Rect"))
        .map(parse_rect)
        .unwrap_or_default();
      widgets.push(PlacedWidget {
        object_id: key
          .map(object_id_string)
          .unwrap_or_else(|| format!("page {page_num} annot {index}")),
        key,
        page: page_num,
        rect,
        dict: dict.clone(),
      });
    }
  }
  widgets
}

/// First `/AP /N` state name that is not `Off`. `None` when there is no
/// appearance dictionary or it only has `Off`; a `Yes` is never fabricated.
fn on_state_of(dict: &Dictionary, doc: &Document, warnings: &mut Vec<String>) -> Option<String> {
  let ap = dict.get(b"AP").ok()?;
  let ap = match deref(doc, ap, warnings, "/AP")? {
    Object::Dictionary(d) => d,
    _ => return None,
  };
  let n = ap.get(b"N").ok()?;
  match deref(doc, n, warnings, "/AP /N")? {
    Object::Dictionary(states) => states
      .iter()
      .map(|(k, _)| k.as_slice())
      .find(|k| *k != b"Off")
      .map(decode_name_bytes),
    _ => None,
  }
}

/// Map a button state name to its export value: `/Opt` may supply the export
/// values by index (ISO 32000-1 §12.7.4.2.3); otherwise the state name is the
/// export value.
fn export_value_for(state: &str, options: Option<&[ChoiceOption]>) -> String {
  if let Some(opts) = options
    && let Ok(idx) = state.parse::<usize>()
    && let Some(opt) = opts.get(idx)
  {
    return opt.export_value.clone();
  }
  state.to_string()
}

/// The widget's own `/AS` state name, if any.
fn appearance_state(
  widget: Option<&PlacedWidget>,
  doc: &Document,
  warnings: &mut Vec<String>,
) -> Option<String> {
  widget.and_then(|w| dict_name(doc, &w.dict, b"AS", warnings))
}

/// The group's `/V` as a state name (buttons store a name; tolerate a string).
fn button_value_name(
  tree: &FieldTree,
  i: usize,
  doc: &Document,
  warnings: &mut Vec<String>,
) -> Option<String> {
  tree
    .effective_value(i)
    .and_then(|v| decode_text_object(v, doc, warnings))
}

fn resolve_kind(
  tree: &FieldTree,
  i: usize,
  widget: Option<&PlacedWidget>,
  doc: &Document,
  warnings: &mut Vec<String>,
) -> FieldKind {
  let flags = tree.effective_flags(i);
  match tree.effective_type(i) {
    Some(FieldType::Text) => FieldKind::Text {
      multiline: flags.multiline(),
      password: flags.password(),
      comb: flags.comb(),
      rich_text: flags.rich_text(),
      max_len: tree.effective_max_len(i),
    },
    Some(FieldType::Button) if flags.push_button() => FieldKind::PushButton {},
    Some(FieldType::Button) => {
      let on_state = widget.and_then(|w| on_state_of(&w.dict, doc, warnings));
      let export_value = on_state
        .as_deref()
        .map(|s| export_value_for(s, tree.effective_options(i)));
      let as_state = appearance_state(widget, doc, warnings);
      let value_name = button_value_name(tree, i, doc, warnings);
      let is_on = |name: &str| name != "Off";
      if flags.radio() {
        // A radio widget is selected by its own /AS. Without /AS the group's /V
        // identifies a state name, which only resolves to this widget when its
        // appearance dictionary names that state; with no appearance states
        // the selection cannot be attributed and is reported as unknown.
        let selected = match (&as_state, &on_state, &value_name) {
          (Some(a), _, _) => is_on(a),
          (None, Some(on), Some(v)) => on == v,
          (None, None, Some(v)) if is_on(v) => {
            if let Some(w) = widget {
              warnings.push(format!(
                "field \"{}\": radio widget {} has no /AS and no appearance states; selection unknown",
                tree.full_name(i),
                w.object_id
              ));
            }
            false
          }
          _ => false,
        };
        FieldKind::Radio {
          on_state,
          export_value,
          selected,
        }
      } else {
        // A checkbox follows its /AS; without one the field value decides,
        // even when the widget has no appearance dictionary to name the state.
        let checked = match (&as_state, &value_name) {
          (Some(a), _) => is_on(a),
          (None, Some(v)) => is_on(v),
          (None, None) => false,
        };
        FieldKind::Checkbox {
          on_state,
          export_value,
          checked,
        }
      }
    }
    Some(FieldType::Choice) => {
      let options = tree
        .effective_options(i)
        .map(|o| o.to_vec())
        .unwrap_or_default();
      if flags.combo() {
        FieldKind::ComboBox {
          options,
          editable: flags.edit(),
        }
      } else {
        let values = tree
          .effective_value(i)
          .and_then(|v| decode_text_array(v, doc, warnings));
        FieldKind::ListBox {
          options,
          multi_select: flags.multi_select(),
          values,
        }
      }
    }
    Some(FieldType::Signature) => {
      let signed = tree
        .effective_value(i)
        .and_then(|v| deref(doc, v, warnings, "/V"))
        .is_some_and(|v| matches!(v, Object::Dictionary(_)));
      FieldKind::Signature { signed }
    }
    None => FieldKind::Unknown {},
  }
}

/// The field's `/V` (and `/DV`) in export form, by type.
fn field_value(
  tree: &FieldTree,
  i: usize,
  obj: Option<&Object>,
  doc: &Document,
  warnings: &mut Vec<String>,
) -> Option<String> {
  match tree.effective_type(i) {
    Some(FieldType::Signature) => None,
    Some(FieldType::Button) => obj
      .and_then(|v| decode_text_object(v, doc, warnings))
      .map(|name| export_value_for(&name, tree.effective_options(i))),
    _ => obj.and_then(|v| decode_text_object(v, doc, warnings)),
  }
}

fn common_for(
  tree: &FieldTree,
  i: usize,
  widget: &PlacedWidget,
  widget_index: u32,
  widget_count: u32,
  doc: &Document,
  warnings: &mut Vec<String>,
) -> FieldCommon {
  let flags = tree.effective_flags(i);
  FieldCommon {
    page: widget.page,
    rect: widget.rect.clone(),
    object_id: widget.object_id.clone(),
    field_object_id: tree.field_object_id(i),
    name: tree.full_name(i),
    partial_name: tree.nodes[i].partial_name.clone(),
    widget_index,
    widget_count,
    value: field_value(tree, i, tree.effective_value(i), doc, warnings),
    default_value: field_value(tree, i, tree.effective_default(i), doc, warnings),
    tooltip: tree.effective_tooltip(i).map(str::to_string),
    mapping_name: tree.effective_mapping_name(i).map(str::to_string),
    read_only: flags.read_only(),
    required: flags.required(),
    no_export: flags.no_export(),
    has_actions: tree.nodes[i].has_actions || widget.dict.has(b"A") || widget.dict.has(b"AA"),
  }
}

/// The full logical payload of a field with no placement (Astra review: an
/// unplaced record keeps every datum a placed one would carry, `signed` included).
fn unplaced_for(
  tree: &FieldTree,
  i: usize,
  doc: &Document,
  warnings: &mut Vec<String>,
) -> UnplacedFormField {
  let flags = tree.effective_flags(i);
  let kind = resolve_kind(tree, i, None, doc, warnings);
  let (
    values,
    options,
    max_len,
    multiline,
    password,
    comb,
    rich_text,
    editable,
    multi_select,
    signed,
  ) = match &kind {
    FieldKind::Text {
      multiline,
      password,
      comb,
      rich_text,
      max_len,
    } => (
      None, None, *max_len, *multiline, *password, *comb, *rich_text, false, false, false,
    ),
    FieldKind::ComboBox { options, editable } => (
      None,
      Some(options.clone()),
      None,
      false,
      false,
      false,
      false,
      *editable,
      false,
      false,
    ),
    FieldKind::ListBox {
      options,
      multi_select,
      values,
    } => (
      values.clone(),
      Some(options.clone()),
      None,
      false,
      false,
      false,
      false,
      false,
      *multi_select,
      false,
    ),
    FieldKind::Signature { signed } => (
      None, None, None, false, false, false, false, false, false, *signed,
    ),
    FieldKind::Checkbox { .. } | FieldKind::Radio { .. } => (
      None,
      tree.effective_options(i).map(|o| o.to_vec()),
      None,
      false,
      false,
      false,
      false,
      false,
      false,
      false,
    ),
    FieldKind::PushButton {} | FieldKind::Unknown {} => (
      None, None, None, false, false, false, false, false, false, false,
    ),
  };
  UnplacedFormField {
    field_object_id: tree.field_object_id(i),
    name: tree.full_name(i),
    partial_name: tree.nodes[i].partial_name.clone(),
    kind: kind.kind_name().to_string(),
    value: field_value(tree, i, tree.effective_value(i), doc, warnings),
    values,
    default_value: field_value(tree, i, tree.effective_default(i), doc, warnings),
    tooltip: tree.effective_tooltip(i).map(str::to_string),
    mapping_name: tree.effective_mapping_name(i).map(str::to_string),
    options,
    max_len,
    read_only: flags.read_only(),
    required: flags.required(),
    no_export: flags.no_export(),
    has_actions: tree.nodes[i].has_actions,
    multiline,
    password,
    comb,
    rich_text,
    editable,
    multi_select,
    signed,
  }
}

/// A widget no field claims: everything comes from its own dictionary.
fn unknown_for(widget: &PlacedWidget, doc: &Document, warnings: &mut Vec<String>) -> PageFormField {
  let flags = FieldFlags(
    dict_int(doc, &widget.dict, b"Ff", warnings)
      .map(|v| v as u32)
      .unwrap_or(0),
  );
  let common = FieldCommon {
    page: widget.page,
    rect: widget.rect.clone(),
    object_id: widget.object_id.clone(),
    field_object_id: widget.object_id.clone(),
    name: dict_text(doc, &widget.dict, b"T", warnings).unwrap_or_default(),
    partial_name: dict_text(doc, &widget.dict, b"T", warnings).unwrap_or_default(),
    widget_index: 0,
    widget_count: 1,
    value: dict_text(doc, &widget.dict, b"V", warnings),
    default_value: dict_text(doc, &widget.dict, b"DV", warnings),
    tooltip: dict_text(doc, &widget.dict, b"TU", warnings),
    mapping_name: dict_text(doc, &widget.dict, b"TM", warnings),
    read_only: flags.read_only(),
    required: flags.required(),
    no_export: flags.no_export(),
    has_actions: widget.dict.has(b"A") || widget.dict.has(b"AA"),
  };
  PageFormField::new(common, FieldKind::Unknown {})
}

// ── Emission ──────────────────────────────────────────────────────

pub(crate) struct FormsExtraction {
  pub info: Option<FormInfo>,
  pub fields: Vec<PageFormField>,
}

fn top_edge(rect: &[f64]) -> f64 {
  rect
    .get(1)
    .copied()
    .unwrap_or(0.0)
    .max(rect.get(3).copied().unwrap_or(0.0))
}

fn left_edge(rect: &[f64]) -> f64 {
  rect
    .first()
    .copied()
    .unwrap_or(0.0)
    .min(rect.get(2).copied().unwrap_or(0.0))
}

/// Page ascending, then top edge descending (reading order), then left edge
/// ascending, then name, then object id. Missing coordinates count as 0.
fn sort_fields(fields: &mut [PageFormField]) {
  fields.sort_by(|a, b| {
    a.page()
      .cmp(&b.page())
      .then_with(|| top_edge(b.rect()).total_cmp(&top_edge(a.rect())))
      .then_with(|| left_edge(a.rect()).total_cmp(&left_edge(b.rect())))
      .then_with(|| a.name().cmp(b.name()))
      .then_with(|| a.object_id().cmp(b.object_id()))
  });
}

/// Sorted unique page numbers that carry at least one placement.
pub(crate) fn form_field_pages(fields: &[PageFormField]) -> Vec<u32> {
  let mut pages: Vec<u32> = fields
    .iter()
    .map(PageFormField::page)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  pages.sort_unstable();
  pages
}

/// The one entry point: all form data of a document.
pub(crate) fn extract_forms(doc: &Document) -> FormsExtraction {
  let mut warnings: Vec<String> = Vec::new();
  let widgets = scan_widgets(doc, &mut warnings);
  let widget_index: HashMap<ObjectId, usize> = widgets
    .iter()
    .enumerate()
    .filter_map(|(i, w)| w.key.map(|k| (k, i)))
    .collect();

  let acroform: Option<&Dictionary> = doc
    .catalog()
    .ok()
    .and_then(|cat| cat.get_deref(b"AcroForm", doc).ok())
    .and_then(|o| o.as_dict().ok());

  let (has_xfa, need_appearances, sig_flags, fields_array) = match acroform {
    Some(af) => {
      let need = af
        .get_deref(b"NeedAppearances", doc)
        .ok()
        .and_then(|o| o.as_bool().ok())
        .unwrap_or(false);
      let sig = dict_int(doc, af, b"SigFlags", &mut warnings)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(0);
      let fields = af
        .get(b"Fields")
        .ok()
        .and_then(|o| deref(doc, o, &mut warnings, "/AcroForm /Fields"))
        .and_then(|o| o.as_array().ok())
        .cloned()
        .unwrap_or_default();
      (af.has(b"XFA"), need, sig, fields)
    }
    None => (false, false, 0, Vec::new()),
  };

  let tree = FieldTree::walk(doc, &fields_array);
  warnings.extend(tree.warnings.iter().cloned());

  let mut fields: Vec<PageFormField> = Vec::new();
  let mut unplaced: Vec<UnplacedFormField> = Vec::new();
  let mut claimed: Vec<bool> = vec![false; widgets.len()];
  let mut field_count = 0u32;

  for i in 0..tree.nodes.len() {
    if !tree.is_logical(i) {
      continue;
    }
    field_count += 1;
    let node = &tree.nodes[i];
    let mut placements: Vec<usize> = Vec::new();
    if let Some(w) = node.object_id.and_then(|id| widget_index.get(&id)) {
      placements.push(*w);
    }
    for &c in &node.children {
      if let Some(w) = tree.nodes[c].object_id.and_then(|id| widget_index.get(&id)) {
        placements.push(*w);
      }
    }
    if placements.is_empty() {
      unplaced.push(unplaced_for(&tree, i, doc, &mut warnings));
      continue;
    }
    let widget_count = placements.len() as u32;
    for (k, &w) in placements.iter().enumerate() {
      claimed[w] = true;
      let widget = &widgets[w];
      let common = common_for(&tree, i, widget, k as u32, widget_count, doc, &mut warnings);
      let kind = resolve_kind(&tree, i, Some(widget), doc, &mut warnings);
      fields.push(PageFormField::new(common, kind));
    }
  }

  for (w, widget) in widgets.iter().enumerate() {
    if !claimed[w] {
      fields.push(unknown_for(widget, doc, &mut warnings));
    }
  }

  sort_fields(&mut fields);

  let info = if acroform.is_some() || !fields.is_empty() {
    Some(FormInfo {
      has_acro_form: acroform.is_some(),
      has_xfa,
      need_appearances,
      sig_flags,
      field_count,
      widget_count: fields.len() as u32,
      unplaced_fields: unplaced,
      truncated: tree.truncated,
      warnings,
    })
  } else {
    None
  };

  FormsExtraction { info, fields }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;
  use lopdf::{Object, StringFormat, dictionary};

  /// A document with `n` empty pages and a catalog; returns the page object ids.
  fn doc_with_pages(n: usize) -> (Document, Vec<ObjectId>) {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let mut kids = Vec::new();
    let mut page_ids = Vec::new();
    for _ in 0..n {
      let pid = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => Object::Reference(pages_id),
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
      });
      kids.push(Object::Reference(pid));
      page_ids.push(pid);
    }
    doc.objects.insert(
      pages_id,
      Object::Dictionary(dictionary! {
        "Type" => "Pages",
        "Kids" => kids,
        "Count" => n as i64,
      }),
    );
    let catalog = doc.add_object(dictionary! {
      "Type" => "Catalog",
      "Pages" => Object::Reference(pages_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog));
    (doc, page_ids)
  }

  fn set_on(doc: &mut Document, id: ObjectId, key: &str, value: Object) {
    if let Some(Object::Dictionary(d)) = doc.objects.get_mut(&id) {
      d.set(key, value);
    } else {
      panic!("{id:?} is not a dictionary");
    }
  }

  fn catalog_id(doc: &Document) -> ObjectId {
    match doc.trailer.get(b"Root") {
      Ok(Object::Reference(id)) => *id,
      _ => panic!("no catalog"),
    }
  }

  fn set_acroform(doc: &mut Document, fields: Vec<Object>) {
    let cat = catalog_id(doc);
    set_on(
      doc,
      cat,
      "AcroForm",
      Object::Dictionary(dictionary! { "Fields" => fields }),
    );
  }

  fn set_annots(doc: &mut Document, page: ObjectId, annots: Object) {
    set_on(doc, page, "Annots", annots);
  }

  fn rect(a: i64, b: i64, c: i64, d: i64) -> Object {
    Object::Array(vec![a.into(), b.into(), c.into(), d.into()])
  }

  fn widget_dict() -> Dictionary {
    dictionary! { "Type" => "Annot", "Subtype" => "Widget", "Rect" => rect(50, 700, 200, 720) }
  }

  fn ap_states(states: &[&str]) -> Object {
    let mut n = Dictionary::new();
    for s in states {
      n.set(*s, Object::Dictionary(Dictionary::new()));
    }
    Object::Dictionary(dictionary! { "N" => Object::Dictionary(n) })
  }

  fn names(fields: &[PageFormField]) -> Vec<String> {
    fields.iter().map(|f| f.name().to_string()).collect()
  }

  // ── Task 2: decoding and tree ──

  #[test]
  fn flags_use_one_based_bit_numbers() {
    let f = FieldFlags(1 << 12 | 1 << 24 | 1);
    assert!(f.multiline() && f.comb() && f.read_only());
    assert!(!f.radio());
  }

  #[test]
  fn object_id_matches_page_image_format() {
    assert_eq!(object_id_string((12, 0)), "12 0 obj");
  }

  #[test]
  fn name_bytes_utf8_then_latin1() {
    assert_eq!(decode_name_bytes("Ja".as_bytes()), "Ja");
    assert_eq!(decode_name_bytes(&[0xF6]), "ö");
  }

  #[test]
  fn text_object_decodes_utf16_pdfdoc_and_numbers() {
    let (doc, _) = doc_with_pages(0);
    let mut w = Vec::new();
    assert_eq!(
      decode_text_object(
        &Object::String(
          vec![0xFE, 0xFF, 0x00, 0x4D, 0x00, 0xFC],
          StringFormat::Hexadecimal
        ),
        &doc,
        &mut w
      ),
      Some("Mü".into())
    );
    assert_eq!(
      decode_text_object(
        &Object::String(b"Stra\xDFe".to_vec(), StringFormat::Literal),
        &doc,
        &mut w
      ),
      Some("Straße".into())
    );
    assert_eq!(
      decode_text_object(&Object::Integer(42), &doc, &mut w),
      Some("42".into())
    );
    assert_eq!(
      decode_text_object(&Object::Dictionary(Dictionary::new()), &doc, &mut w),
      None
    );
    assert!(w.is_empty());
  }

  #[test]
  fn indirect_values_resolve_and_dangling_refs_warn() {
    let (mut doc, _) = doc_with_pages(0);
    let s = doc.add_object(Object::string_literal("via ref"));
    let a = doc.add_object(Object::Array(vec![
      Object::string_literal("a"),
      Object::string_literal("c"),
    ]));
    let mut w = Vec::new();
    assert_eq!(
      decode_text_object(&Object::Reference(s), &doc, &mut w),
      Some("via ref".into())
    );
    assert_eq!(
      decode_text_object(&Object::Reference(a), &doc, &mut w),
      Some("a".into())
    );
    assert_eq!(
      decode_text_object(&Object::Reference((999, 0)), &doc, &mut w),
      None
    );
    assert_eq!(w.len(), 1);
    assert!(w[0].contains("999 0 R"));
  }

  #[test]
  fn inheritance_resolves_type_flags_value_through_parent() {
    let (mut doc, _) = doc_with_pages(0);
    let parent = doc.new_object_id();
    let kid = doc.add_object(
      dictionary! { "T" => Object::string_literal("kid"), "Parent" => Object::Reference(parent) },
    );
    doc.objects.insert(
      parent,
      Object::Dictionary(dictionary! {
        "T" => Object::string_literal("parent"), "FT" => "Tx", "Ff" => 1,
        "V" => Object::string_literal("shared"), "Kids" => vec![Object::Reference(kid)],
      }),
    );
    let tree = FieldTree::walk(&doc, &[Object::Reference(parent)]);
    assert_eq!(tree.nodes.len(), 2);
    assert_eq!(tree.effective_type(1), Some(FieldType::Text));
    assert!(tree.effective_flags(1).read_only());
    let mut w = Vec::new();
    assert_eq!(
      tree
        .effective_value(1)
        .and_then(|v| decode_text_object(v, &doc, &mut w)),
      Some("shared".into())
    );
    assert_eq!(tree.full_name(1), "parent.kid");
  }

  #[test]
  fn container_with_named_child_is_not_logical_but_child_is() {
    let (mut doc, _) = doc_with_pages(0);
    let parent = doc.new_object_id();
    let mut kd = widget_dict();
    kd.set("T", Object::string_literal("kid"));
    kd.set("Parent", Object::Reference(parent));
    let kid = doc.add_object(kd);
    doc.objects.insert(
      parent,
      Object::Dictionary(dictionary! { "T" => Object::string_literal("p"), "FT" => "Tx", "Kids" => vec![Object::Reference(kid)] }),
    );
    let tree = FieldTree::walk(&doc, &[Object::Reference(parent)]);
    assert!(!tree.is_logical(0));
    assert!(tree.is_logical(1));
  }

  #[test]
  fn unnamed_kids_make_parent_logical() {
    let (mut doc, _) = doc_with_pages(0);
    let group = doc.new_object_id();
    let k1 = doc.add_object(dictionary! { "Subtype" => "Widget", "Rect" => rect(0, 0, 1, 1), "Parent" => Object::Reference(group) });
    let k2 = doc.add_object(dictionary! { "Subtype" => "Widget", "Rect" => rect(0, 0, 1, 1), "Parent" => Object::Reference(group) });
    doc.objects.insert(
      group,
      Object::Dictionary(dictionary! {
        "T" => Object::string_literal("color"), "FT" => "Btn", "Ff" => 1 << 15,
        "Kids" => vec![Object::Reference(k1), Object::Reference(k2)],
      }),
    );
    let tree = FieldTree::walk(&doc, &[Object::Reference(group)]);
    assert!(tree.is_logical(0));
    assert_eq!(tree.effective_type(1), Some(FieldType::Button));
    assert!(!tree.is_logical(1));
    assert!(!tree.is_logical(2));
  }

  #[test]
  fn unnamed_root_field_is_still_logical() {
    let (mut doc, _) = doc_with_pages(0);
    let f = doc.add_object(dictionary! { "FT" => "Tx" });
    let tree = FieldTree::walk(&doc, &[Object::Reference(f)]);
    assert!(tree.nodes[0].is_root);
    assert!(tree.is_logical(0));
    assert_eq!(tree.full_name(0), "");
  }

  #[test]
  fn inline_kid_gets_synthetic_field_object_id() {
    let (mut doc, _) = doc_with_pages(0);
    let parent = doc.add_object(dictionary! {
      "T" => Object::string_literal("p"), "FT" => "Tx",
      "Kids" => vec![Object::Dictionary(widget_dict())],
    });
    let tree = FieldTree::walk(&doc, &[Object::Reference(parent)]);
    assert_eq!(tree.nodes[1].inline_index, Some(0));
    assert_eq!(
      tree.field_object_id(1),
      format!("{}/kid 0", object_id_string(parent))
    );
    assert_eq!(tree.field_object_id(0), object_id_string(parent));
  }

  #[test]
  fn kids_cycle_terminates_and_dedupes() {
    let (mut doc, _) = doc_with_pages(0);
    let a = doc.new_object_id();
    let b = doc.new_object_id();
    doc.objects.insert(a, Object::Dictionary(dictionary! { "T" => Object::string_literal("A"), "FT" => "Tx", "Kids" => vec![Object::Reference(b)] }));
    doc.objects.insert(
      b,
      Object::Dictionary(
        dictionary! { "T" => Object::string_literal("B"), "Kids" => vec![Object::Reference(a)] },
      ),
    );
    let tree = FieldTree::walk(&doc, &[Object::Reference(a)]);
    assert_eq!(tree.nodes.len(), 2);
    assert!(tree.nodes[1].children.is_empty());
    assert!(!tree.truncated);
  }

  #[test]
  fn depth_is_capped_and_reported() {
    let (mut doc, _) = doc_with_pages(0);
    let mut ids: Vec<ObjectId> = (0..150).map(|_| doc.new_object_id()).collect();
    ids.reverse();
    let mut child: Option<ObjectId> = None;
    for id in &ids {
      let mut d = dictionary! { "T" => Object::string_literal("n") };
      if let Some(c) = child {
        d.set("Kids", vec![Object::Reference(c)]);
      }
      doc.objects.insert(*id, Object::Dictionary(d));
      child = Some(*id);
    }
    let root = child.unwrap();
    set_on(&mut doc, root, "FT", "Tx".into());
    let tree = FieldTree::walk(&doc, &[Object::Reference(root)]);
    assert!(tree.nodes.len() <= MAX_DEPTH);
    assert!(tree.truncated);
    assert!(tree.warnings.iter().any(|w| w.contains("depth")));
  }

  #[test]
  fn bad_kids_entries_warn() {
    let (mut doc, _) = doc_with_pages(0);
    let f = doc.add_object(dictionary! {
      "T" => Object::string_literal("f"), "FT" => "Tx",
      "Kids" => vec![Object::Integer(42), Object::Reference((777, 0))],
    });
    let tree = FieldTree::walk(&doc, &[Object::Reference(f)]);
    assert_eq!(tree.nodes.len(), 1);
    assert_eq!(tree.warnings.len(), 2);
    assert!(!tree.truncated);
  }

  #[test]
  fn options_accept_strings_and_pairs() {
    let (doc, _) = doc_with_pages(0);
    let mut w = Vec::new();
    let opts = parse_options(
      &doc,
      &Object::Array(vec![
        Object::string_literal("a"),
        Object::Array(vec![
          Object::string_literal("US"),
          Object::string_literal("United States"),
        ]),
      ]),
      &mut w,
    );
    assert_eq!(opts.len(), 2);
    assert_eq!(
      (
        opts[0].export_value.as_str(),
        opts[0].display_value.as_str()
      ),
      ("a", "a")
    );
    assert_eq!(
      (
        opts[1].export_value.as_str(),
        opts[1].display_value.as_str()
      ),
      ("US", "United States")
    );
  }

  #[test]
  fn rect_keeps_numeric_entries_in_order() {
    assert_eq!(
      parse_rect(&Object::Array(vec![
        Object::Integer(0),
        Object::Name(b"x".to_vec()),
        Object::Real(100.5)
      ])),
      vec![0.0, 100.5]
    );
  }

  // ── Task 3: widgets and kinds ──

  #[test]
  fn scan_finds_widgets_behind_indirect_annots_and_inline_dicts() {
    let (mut doc, pages) = doc_with_pages(2);
    let w1 = doc.add_object(widget_dict());
    let link = doc.add_object(dictionary! { "Subtype" => "Link", "Rect" => rect(0, 0, 1, 1) });
    let arr = doc.add_object(Object::Array(vec![
      Object::Reference(w1),
      Object::Reference(link),
    ]));
    set_annots(&mut doc, pages[0], Object::Reference(arr));
    set_annots(
      &mut doc,
      pages[1],
      Object::Array(vec![Object::Dictionary(widget_dict())]),
    );
    let mut warnings = Vec::new();
    let widgets = scan_widgets(&doc, &mut warnings);
    assert_eq!(widgets.len(), 2);
    assert_eq!(
      widgets.iter().map(|w| w.page).collect::<Vec<_>>(),
      vec![1, 2]
    );
    assert_eq!(widgets[1].object_id, "page 2 annot 0");
    assert!(widgets[1].key.is_none());
    assert!(warnings.is_empty());
  }

  #[test]
  fn on_state_is_first_non_off_ap_key() {
    let (doc, _) = doc_with_pages(0);
    let mut w = Vec::new();
    let mut d = widget_dict();
    d.set("AP", ap_states(&["Off", "Ja"]));
    assert_eq!(on_state_of(&d, &doc, &mut w), Some("Ja".into()));
    d.set("AP", ap_states(&["Off"]));
    assert_eq!(on_state_of(&d, &doc, &mut w), None);
    assert_eq!(on_state_of(&widget_dict(), &doc, &mut w), None);
  }

  #[test]
  fn export_value_for_maps_indices_through_opt() {
    let o = [
      ChoiceOption {
        export_value: "Red".into(),
        display_value: "Red".into(),
      },
      ChoiceOption {
        export_value: "Green".into(),
        display_value: "Green".into(),
      },
    ];
    assert_eq!(export_value_for("1", Some(&o)), "Green");
    assert_eq!(export_value_for("Blue", Some(&o)), "Blue");
    assert_eq!(export_value_for("1", None), "1");
  }

  /// Builds one checkbox field placed on page 1 with the given /AS, /V and /AP.
  fn checkbox_doc(
    as_state: Option<&str>,
    value: Option<&str>,
    ap: bool,
  ) -> (Document, FieldTree, Vec<PlacedWidget>) {
    let (mut doc, pages) = doc_with_pages(1);
    let mut d = widget_dict();
    d.set("T", Object::string_literal("agree"));
    d.set("FT", "Btn");
    if let Some(a) = as_state {
      d.set("AS", a);
    }
    if let Some(v) = value {
      d.set("V", v);
    }
    if ap {
      d.set("AP", ap_states(&["Off", "Ja"]));
    }
    let id = doc.add_object(d);
    set_annots(
      &mut doc,
      pages[0],
      Object::Array(vec![Object::Reference(id)]),
    );
    let tree = FieldTree::walk(&doc, &[Object::Reference(id)]);
    let mut w = Vec::new();
    let widgets = scan_widgets(&doc, &mut w);
    (doc, tree, widgets)
  }

  fn checkbox_state(
    doc: &Document,
    tree: &FieldTree,
    widgets: &[PlacedWidget],
  ) -> (Option<String>, Option<String>, bool, Option<String>) {
    let mut w = Vec::new();
    let kind = resolve_kind(tree, 0, widgets.first(), doc, &mut w);
    let common = common_for(tree, 0, &widgets[0], 0, 1, doc, &mut w);
    match kind {
      FieldKind::Checkbox {
        on_state,
        export_value,
        checked,
      } => (on_state, export_value, checked, common.value),
      other => panic!("expected Checkbox, got {}", other.kind_name()),
    }
  }

  #[test]
  fn checkbox_checked_from_as_then_v_and_disagreement_is_visible() {
    let (doc, tree, widgets) = checkbox_doc(Some("Ja"), None, true);
    assert!(checkbox_state(&doc, &tree, &widgets).2);
    let (doc, tree, widgets) = checkbox_doc(None, Some("Ja"), true);
    assert!(checkbox_state(&doc, &tree, &widgets).2);
    let (doc, tree, widgets) = checkbox_doc(Some("Off"), Some("Ja"), true);
    let (_, _, checked, value) = checkbox_state(&doc, &tree, &widgets);
    assert!(!checked);
    assert_eq!(value.as_deref(), Some("Ja"));
  }

  #[test]
  fn checkbox_without_appearance_has_no_state_names() {
    let (doc, tree, widgets) = checkbox_doc(None, Some("Ja"), false);
    let (on_state, export_value, checked, value) = checkbox_state(&doc, &tree, &widgets);
    assert_eq!(on_state, None);
    assert_eq!(export_value, None);
    assert!(checked);
    assert_eq!(value.as_deref(), Some("Ja"));
  }

  /// Radio group with three kids whose /AP keys are /0 /1 /2 and the given /AS states.
  fn radio_doc(
    as_states: [Option<&str>; 3],
    with_ap: bool,
  ) -> (Document, FieldTree, Vec<PlacedWidget>) {
    let (mut doc, pages) = doc_with_pages(1);
    let group = doc.new_object_id();
    let mut kids = Vec::new();
    for (i, a) in as_states.iter().enumerate() {
      let mut d = widget_dict();
      d.set(
        "Rect",
        rect(50 + 50 * i as i64, 480, 70 + 50 * i as i64, 500),
      );
      d.set("Parent", Object::Reference(group));
      if with_ap {
        d.set("AP", ap_states(&["Off", &i.to_string()]));
      }
      if let Some(a) = a {
        d.set("AS", *a);
      }
      kids.push(Object::Reference(doc.add_object(d)));
    }
    doc.objects.insert(
      group,
      Object::Dictionary(dictionary! {
        "T" => Object::string_literal("color"), "FT" => "Btn", "Ff" => 1 << 15, "V" => "1",
        "Opt" => vec![Object::string_literal("Red"), Object::string_literal("Green"), Object::string_literal("Blue")],
        "Kids" => kids.clone(),
      }),
    );
    set_annots(&mut doc, pages[0], Object::Array(kids));
    let tree = FieldTree::walk(&doc, &[Object::Reference(group)]);
    let mut w = Vec::new();
    let widgets = scan_widgets(&doc, &mut w);
    (doc, tree, widgets)
  }

  #[test]
  fn radio_keeps_state_name_and_export_value_apart() {
    let (doc, tree, widgets) = radio_doc([Some("Off"), Some("1"), Some("Off")], true);
    let mut w = Vec::new();
    let mut seen = Vec::new();
    for widget in &widgets {
      let kind = resolve_kind(&tree, 0, Some(widget), &doc, &mut w);
      let common = common_for(&tree, 0, widget, 0, 3, &doc, &mut w);
      match kind {
        FieldKind::Radio {
          on_state,
          export_value,
          selected,
        } => seen.push((on_state, export_value, selected, common.value)),
        other => panic!("expected Radio, got {}", other.kind_name()),
      }
    }
    assert_eq!(
      seen
        .iter()
        .map(|s| s.0.clone().unwrap())
        .collect::<Vec<_>>(),
      ["0", "1", "2"]
    );
    assert_eq!(
      seen
        .iter()
        .map(|s| s.1.clone().unwrap())
        .collect::<Vec<_>>(),
      ["Red", "Green", "Blue"]
    );
    assert_eq!(
      seen.iter().map(|s| s.2).collect::<Vec<_>>(),
      [false, true, false]
    );
    assert!(seen.iter().all(|s| s.3.as_deref() == Some("Green")));
    assert!(w.is_empty());
  }

  #[test]
  fn radio_without_as_resolves_through_appearance_states_only() {
    // No /AS anywhere: the group /V "1" selects the widget whose /AP names state "1".
    let (doc, tree, widgets) = radio_doc([None, None, None], true);
    let mut w = Vec::new();
    let selected: Vec<bool> = widgets
      .iter()
      .map(
        |widget| match resolve_kind(&tree, 0, Some(widget), &doc, &mut w) {
          FieldKind::Radio { selected, .. } => selected,
          _ => unreachable!(),
        },
      )
      .collect();
    assert_eq!(selected, [false, true, false]);
    assert!(w.is_empty());
    // No /AS and no /AP: the group value cannot be attributed to a widget.
    let (doc, tree, widgets) = radio_doc([None, None, None], false);
    let mut w = Vec::new();
    let selected: Vec<bool> = widgets
      .iter()
      .map(
        |widget| match resolve_kind(&tree, 0, Some(widget), &doc, &mut w) {
          FieldKind::Radio {
            selected, on_state, ..
          } => {
            assert!(on_state.is_none());
            selected
          }
          _ => unreachable!(),
        },
      )
      .collect();
    assert_eq!(selected, [false, false, false]);
    assert_eq!(w.len(), 3);
    assert!(w[0].contains("selection unknown"));
  }

  #[test]
  fn choice_kinds_and_values() {
    let (mut doc, _) = doc_with_pages(0);
    let combo = doc.add_object(dictionary! {
      "T" => Object::string_literal("country"), "FT" => "Ch", "Ff" => (1 << 17) | (1 << 18),
      "Opt" => vec![Object::string_literal("US")], "V" => Object::string_literal("US"),
    });
    let list = doc.add_object(dictionary! {
      "T" => Object::string_literal("tags"), "FT" => "Ch", "Ff" => 1 << 21,
      "Opt" => vec![Object::string_literal("a"), Object::string_literal("b"), Object::string_literal("c")],
      "V" => vec![Object::string_literal("a"), Object::string_literal("c")],
    });
    let tree = FieldTree::walk(&doc, &[Object::Reference(combo), Object::Reference(list)]);
    let mut w = Vec::new();
    match resolve_kind(&tree, 0, None, &doc, &mut w) {
      FieldKind::ComboBox { options, editable } => {
        assert!(editable);
        assert_eq!(options.len(), 1);
      }
      other => panic!("{}", other.kind_name()),
    }
    match resolve_kind(&tree, 1, None, &doc, &mut w) {
      FieldKind::ListBox {
        multi_select,
        values,
        ..
      } => {
        assert!(multi_select);
        assert_eq!(values, Some(vec!["a".into(), "c".into()]));
      }
      other => panic!("{}", other.kind_name()),
    }
    assert_eq!(
      field_value(&tree, 1, tree.effective_value(1), &doc, &mut w).as_deref(),
      Some("a")
    );
  }

  #[test]
  fn signature_signed_only_when_v_resolves_to_a_dictionary() {
    let (mut doc, _) = doc_with_pages(0);
    let sig_dict = doc.add_object(dictionary! { "Type" => "Sig" });
    let sig_str = doc.add_object(Object::string_literal("not a signature"));
    let a = doc.add_object(dictionary! { "T" => Object::string_literal("a"), "FT" => "Sig", "V" => Object::Reference(sig_dict) });
    let b = doc.add_object(dictionary! { "T" => Object::string_literal("b"), "FT" => "Sig", "V" => Object::Reference(sig_str) });
    let c = doc.add_object(dictionary! { "T" => Object::string_literal("c"), "FT" => "Sig", "V" => Object::Reference((999, 0)) });
    let d = doc.add_object(dictionary! { "T" => Object::string_literal("d"), "FT" => "Sig" });
    let tree = FieldTree::walk(&doc, &[a, b, c, d].map(Object::Reference));
    let mut w = Vec::new();
    let signed: Vec<bool> = (0..4)
      .map(|i| match resolve_kind(&tree, i, None, &doc, &mut w) {
        FieldKind::Signature { signed } => signed,
        _ => unreachable!(),
      })
      .collect();
    assert_eq!(signed, [true, false, false, false]);
    assert_eq!(w.len(), 1);
    assert!((0..4).all(|i| field_value(&tree, i, tree.effective_value(i), &doc, &mut w).is_none()));
  }

  #[test]
  fn choice_values_resolve_indirect_arrays() {
    let (mut doc, _) = doc_with_pages(0);
    let v = doc.add_object(Object::Array(vec![
      Object::string_literal("a"),
      Object::string_literal("c"),
    ]));
    let opt = doc.add_object(Object::Array(vec![
      Object::string_literal("a"),
      Object::string_literal("c"),
    ]));
    let list = doc.add_object(dictionary! {
      "T" => Object::string_literal("tags"), "FT" => "Ch", "Ff" => 1 << 21,
      "Opt" => Object::Reference(opt), "V" => Object::Reference(v),
    });
    let tree = FieldTree::walk(&doc, &[Object::Reference(list)]);
    let mut w = Vec::new();
    match resolve_kind(&tree, 0, None, &doc, &mut w) {
      FieldKind::ListBox {
        options, values, ..
      } => {
        assert_eq!(options.len(), 2);
        assert_eq!(values, Some(vec!["a".into(), "c".into()]));
      }
      _ => unreachable!(),
    }
  }

  #[test]
  fn non_string_values_and_actions() {
    let (mut doc, pages) = doc_with_pages(1);
    let mut d = widget_dict();
    d.set("T", Object::string_literal("n"));
    d.set("FT", "Tx");
    d.set("V", 42);
    d.set("A", Object::Dictionary(dictionary! { "S" => "ResetForm" }));
    let n = doc.add_object(d);
    let e = doc.add_object(dictionary! { "T" => Object::string_literal("e"), "FT" => "Tx", "V" => Object::Dictionary(Dictionary::new()) });
    set_annots(
      &mut doc,
      pages[0],
      Object::Array(vec![Object::Reference(n)]),
    );
    let tree = FieldTree::walk(&doc, &[Object::Reference(n), Object::Reference(e)]);
    let mut w = Vec::new();
    let widgets = scan_widgets(&doc, &mut w);
    let common = common_for(&tree, 0, &widgets[0], 0, 1, &doc, &mut w);
    assert_eq!(common.value.as_deref(), Some("42"));
    assert!(common.has_actions);
    assert_eq!(
      field_value(&tree, 1, tree.effective_value(1), &doc, &mut w),
      None
    );
  }

  // ── Task 4: extraction ──

  #[test]
  fn no_acroform_no_widgets_gives_none_and_empty() {
    let (doc, _) = doc_with_pages(1);
    let out = extract_forms(&doc);
    assert!(out.info.is_none());
    assert!(out.fields.is_empty());
  }

  #[test]
  fn widgets_without_acroform_become_unknown_with_info() {
    let (mut doc, pages) = doc_with_pages(1);
    let mut a = widget_dict();
    a.set("T", Object::string_literal("ghost"));
    a.set("V", Object::string_literal("boo"));
    let mut b = widget_dict();
    b.set("T", Object::string_literal("inline"));
    b.set("Rect", rect(50, 600, 200, 620));
    let a = doc.add_object(a);
    set_annots(
      &mut doc,
      pages[0],
      Object::Array(vec![Object::Reference(a), Object::Dictionary(b)]),
    );
    let out = extract_forms(&doc);
    let info = out.info.expect("info present when widgets exist");
    assert!(!info.has_acro_form);
    assert_eq!(info.field_count, 0);
    assert_eq!(info.widget_count, 2);
    assert_eq!(names(&out.fields), ["ghost", "inline"]);
    assert!(out.fields.iter().all(|f| f.kind_name() == "Unknown"));
    assert_eq!(out.fields[1].object_id(), "page 1 annot 1");
  }

  #[test]
  fn unplaced_field_goes_to_info_not_array() {
    let (mut doc, _) = doc_with_pages(1);
    let hidden = doc.add_object(dictionary! { "T" => Object::string_literal("hidden"), "FT" => "Tx", "V" => Object::string_literal("secret") });
    set_acroform(&mut doc, vec![Object::Reference(hidden)]);
    let out = extract_forms(&doc);
    assert!(out.fields.is_empty());
    let info = out.info.unwrap();
    assert_eq!(info.field_count, 1);
    assert_eq!(info.widget_count, 0);
    assert_eq!(info.unplaced_fields.len(), 1);
    let u = &info.unplaced_fields[0];
    assert_eq!(
      (u.name.as_str(), u.kind.as_str(), u.value.as_deref()),
      ("hidden", "Text", Some("secret"))
    );
    assert_eq!(u.field_object_id, object_id_string(hidden));
  }

  #[test]
  fn unplaced_multiselect_keeps_values_options_and_flags() {
    let (mut doc, _) = doc_with_pages(1);
    let list = doc.add_object(dictionary! {
      "T" => Object::string_literal("tags"), "FT" => "Ch", "Ff" => 1 << 21,
      "Opt" => vec![Object::string_literal("a"), Object::string_literal("b"), Object::string_literal("c")],
      "V" => vec![Object::string_literal("a"), Object::string_literal("c")],
    });
    set_acroform(&mut doc, vec![Object::Reference(list)]);
    let info = extract_forms(&doc).info.unwrap();
    let u = &info.unplaced_fields[0];
    assert_eq!(u.kind, "ListBox");
    assert_eq!(u.values, Some(vec!["a".into(), "c".into()]));
    assert_eq!(u.options.as_ref().map(Vec::len), Some(3));
    assert!(u.multi_select);
    assert_eq!(u.value.as_deref(), Some("a"));
  }

  #[test]
  fn unplaced_signature_keeps_signed() {
    let (mut doc, _) = doc_with_pages(1);
    let sig = doc.add_object(dictionary! { "Type" => "Sig" });
    let f = doc.add_object(dictionary! { "T" => Object::string_literal("sealed"), "FT" => "Sig", "V" => Object::Reference(sig) });
    set_acroform(&mut doc, vec![Object::Reference(f)]);
    let info = extract_forms(&doc).info.unwrap();
    let u = &info.unplaced_fields[0];
    assert_eq!(u.kind, "Signature");
    assert!(u.signed);
    assert_eq!(u.value, None);
  }

  #[test]
  fn inline_kid_dictionary_is_unplaced_under_the_logical_field_id() {
    // Astra review: the inline kid is the widget; the logical field keeps its own id.
    let (mut doc, _) = doc_with_pages(1);
    let parent = doc.add_object(dictionary! {
      "T" => Object::string_literal("inlinekid"), "FT" => "Tx",
      "Kids" => vec![Object::Dictionary(widget_dict())],
    });
    set_acroform(&mut doc, vec![Object::Reference(parent)]);
    let out = extract_forms(&doc);
    assert!(out.fields.is_empty());
    let info = out.info.unwrap();
    assert_eq!(info.unplaced_fields.len(), 1);
    assert_eq!(
      info.unplaced_fields[0].field_object_id,
      object_id_string(parent)
    );
    assert_eq!(info.field_count, 1);
  }

  #[test]
  fn truncation_and_dangling_refs_surface_in_info() {
    let (mut doc, _) = doc_with_pages(1);
    let mut ids: Vec<ObjectId> = (0..150).map(|_| doc.new_object_id()).collect();
    ids.reverse();
    let mut child: Option<ObjectId> = None;
    for id in &ids {
      let mut d = dictionary! { "T" => Object::string_literal("n") };
      if let Some(c) = child {
        d.set(
          "Kids",
          vec![Object::Reference(c), Object::Reference((888, 0))],
        );
      }
      doc.objects.insert(*id, Object::Dictionary(d));
      child = Some(*id);
    }
    let root = child.unwrap();
    set_on(&mut doc, root, "FT", "Tx".into());
    set_acroform(&mut doc, vec![Object::Reference(root)]);
    let info = extract_forms(&doc).info.unwrap();
    assert!(info.truncated);
    assert!(info.warnings.len() >= 2);
  }

  #[test]
  fn multi_page_field_emits_one_entry_per_placement() {
    let (mut doc, pages) = doc_with_pages(3);
    let field = doc.new_object_id();
    let mut k1 = widget_dict();
    k1.set("Parent", Object::Reference(field));
    let mut k2 = widget_dict();
    k2.set("Parent", Object::Reference(field));
    let k1 = doc.add_object(k1);
    let k2 = doc.add_object(k2);
    doc.objects.insert(
      field,
      Object::Dictionary(dictionary! {
        "T" => Object::string_literal("id"), "FT" => "Tx", "V" => Object::string_literal("A-1"),
        "Kids" => vec![Object::Reference(k1), Object::Reference(k2)],
      }),
    );
    set_annots(
      &mut doc,
      pages[1],
      Object::Array(vec![Object::Reference(k1)]),
    );
    set_annots(
      &mut doc,
      pages[2],
      Object::Array(vec![Object::Reference(k2)]),
    );
    set_acroform(&mut doc, vec![Object::Reference(field)]);
    let out = extract_forms(&doc);
    assert_eq!(out.fields.len(), 2);
    assert_eq!(form_field_pages(&out.fields), vec![2, 3]);
    let ids: Vec<&str> = out
      .fields
      .iter()
      .map(|f| match f {
        PageFormField::Text {
          field_object_id, ..
        } => field_object_id.as_str(),
        _ => unreachable!(),
      })
      .collect();
    assert_eq!(ids[0], ids[1]);
    let idx: Vec<(u32, u32)> = out
      .fields
      .iter()
      .map(|f| match f {
        PageFormField::Text {
          widget_index,
          widget_count,
          ..
        } => (*widget_index, *widget_count),
        _ => unreachable!(),
      })
      .collect();
    assert_eq!(idx, [(0, 2), (1, 2)]);
  }

  #[test]
  fn duplicate_names_keep_distinct_field_object_ids() {
    let (mut doc, pages) = doc_with_pages(1);
    let mut a = widget_dict();
    a.set("T", Object::string_literal("dup"));
    a.set("FT", "Tx");
    let mut b = a.clone();
    b.set("Rect", rect(50, 560, 200, 580));
    let a = doc.add_object(a);
    let b = doc.add_object(b);
    set_annots(
      &mut doc,
      pages[0],
      Object::Array(vec![Object::Reference(a), Object::Reference(b)]),
    );
    set_acroform(&mut doc, vec![Object::Reference(a), Object::Reference(b)]);
    let out = extract_forms(&doc);
    assert_eq!(names(&out.fields), ["dup", "dup"]);
    let ids: Vec<String> = out
      .fields
      .iter()
      .map(|f| match f {
        PageFormField::Text {
          field_object_id, ..
        } => field_object_id.clone(),
        _ => unreachable!(),
      })
      .collect();
    assert_ne!(ids[0], ids[1]);
  }

  #[test]
  fn sort_is_page_then_top_down_then_left_right_then_name() {
    let mk = |page: u32, rect: Vec<f64>, name: &str| {
      PageFormField::new(
        FieldCommon {
          page,
          rect,
          name: name.into(),
          ..FieldCommon::default()
        },
        FieldKind::Unknown {},
      )
    };
    let mut v = vec![
      mk(2, vec![0.0, 0.0, 10.0, 10.0], "p2"),
      mk(1, vec![50.0, 600.0, 200.0, 620.0], "lower"),
      mk(1, vec![250.0, 700.0, 400.0, 720.0], "top-right"),
      mk(1, vec![50.0, 700.0, 200.0, 720.0], "top-left"),
      mk(1, vec![0.0, 0.0, 100.0], "badrect"),
    ];
    sort_fields(&mut v);
    assert_eq!(
      names(&v),
      ["top-left", "top-right", "lower", "badrect", "p2"]
    );
  }

  #[test]
  fn field_count_excludes_unknown_and_counts_unplaced() {
    let (mut doc, pages) = doc_with_pages(1);
    let mut placed = widget_dict();
    placed.set("T", Object::string_literal("placed"));
    placed.set("FT", "Tx");
    let placed = doc.add_object(placed);
    let unplaced =
      doc.add_object(dictionary! { "T" => Object::string_literal("hidden"), "FT" => "Tx" });
    let mut orphan = widget_dict();
    orphan.set("Rect", rect(50, 600, 200, 620));
    let orphan = doc.add_object(orphan);
    set_annots(
      &mut doc,
      pages[0],
      Object::Array(vec![Object::Reference(placed), Object::Reference(orphan)]),
    );
    set_acroform(
      &mut doc,
      vec![Object::Reference(placed), Object::Reference(unplaced)],
    );
    let info = extract_forms(&doc).info.unwrap();
    assert_eq!(info.field_count, 2);
    assert_eq!(info.widget_count, 2);
  }

  #[test]
  fn acroform_dictionary_flags_are_reported() {
    let (mut doc, _) = doc_with_pages(1);
    let cat = catalog_id(&doc);
    set_on(
      &mut doc,
      cat,
      "AcroForm",
      Object::Dictionary(dictionary! {
        "Fields" => Vec::<Object>::new(), "NeedAppearances" => true, "SigFlags" => 3, "XFA" => Vec::<Object>::new(),
      }),
    );
    let info = extract_forms(&doc).info.unwrap();
    assert!(info.need_appearances && info.has_xfa);
    assert_eq!(info.sig_flags, 3);
    let (mut doc, _) = doc_with_pages(1);
    set_acroform(&mut doc, Vec::new());
    let info = extract_forms(&doc).info.unwrap();
    assert!(!info.need_appearances && !info.has_xfa);
    assert_eq!(info.sig_flags, 0);
    assert!(info.has_acro_form);
  }
}
