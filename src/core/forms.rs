//! Read-only AcroForm field extraction (spec: notes/2026-11-01.md §5).
//!
//! This module walks `/AcroForm /Fields` into an arena of [`FieldNode`]s, resolves the
//! inheritable attributes once per logical field, and (in later tasks) joins fields to the
//! widget annotations placed on pages. Nothing here writes to the document.

use crate::types::{
  ChoiceOption, FieldCommon, FieldKind, FormInfo, PageFormField, UnplacedFormField,
};
use lopdf::{Dictionary, Document, Object, ObjectId};
use std::collections::{BTreeSet, HashMap, HashSet};

/// Bounds `/Kids` recursion on malformed or adversarial field trees; real hierarchies are shallow.
const MAX_FIELD_DEPTH: usize = 100;

/// `"N G obj"`, the same format `PageImage.objectId` uses.
pub(crate) fn object_id_string(id: ObjectId) -> String {
  format!("{} {} obj", id.0, id.1)
}

/// `/Name` bytes are not text strings: try UTF-8, else Latin-1 byte-for-byte (lossless).
pub(crate) fn decode_name_bytes(bytes: &[u8]) -> String {
  match std::str::from_utf8(bytes) {
    Ok(s) => s.to_owned(),
    Err(_) => bytes.iter().map(|&b| b as char).collect(),
  }
}

/// One level of indirect-reference resolution. A dangling reference is reported in `warnings`
/// and treated as absent; anything that is not a reference passes through unchanged.
pub(crate) fn deref<'a>(
  doc: &'a Document,
  obj: &'a Object,
  warnings: &mut Vec<String>,
  what: &str,
) -> Option<&'a Object> {
  match obj {
    Object::Reference(id) => match doc.get_object(*id) {
      Ok(resolved) => Some(resolved),
      Err(_) => {
        warnings.push(format!("{what}: unresolved reference {} {} R", id.0, id.1));
        None
      }
    },
    other => Some(other),
  }
}

/// Decode a PDF text-ish object to a `String`: text strings (UTF-16BE BOM, UTF-8 BOM,
/// PDFDocEncoding), names, numbers, and the first element of an array. Dictionaries, streams
/// and everything else yield `None`.
pub(crate) fn decode_text_object(
  obj: &Object,
  doc: &Document,
  warnings: &mut Vec<String>,
) -> Option<String> {
  let obj = deref(doc, obj, warnings, "value")?;
  match obj {
    Object::String(bytes, _) => Some(
      lopdf::decode_text_string(obj)
        .unwrap_or_else(|_| String::from_utf8_lossy(bytes).into_owned()),
    ),
    Object::Name(n) => Some(decode_name_bytes(n)),
    Object::Integer(i) => Some(i.to_string()),
    Object::Real(r) => Some(r.to_string()),
    Object::Array(items) => items
      .first()
      .and_then(|first| decode_text_object(first, doc, warnings)),
    _ => None,
  }
}

/// `/Ff` field flags (ISO 32000-1 tables 221, 226, 227, 230). Bit N means `1 << (N - 1)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct FieldFlags(pub u32);

impl FieldFlags {
  fn bit(self, n: u32) -> bool {
    self.0 & (1 << (n - 1)) != 0
  }
  pub(crate) fn read_only(self) -> bool {
    self.bit(1)
  }
  pub(crate) fn required(self) -> bool {
    self.bit(2)
  }
  pub(crate) fn no_export(self) -> bool {
    self.bit(3)
  }
  pub(crate) fn multiline(self) -> bool {
    self.bit(13)
  }
  pub(crate) fn password(self) -> bool {
    self.bit(14)
  }
  pub(crate) fn radio(self) -> bool {
    self.bit(16)
  }
  pub(crate) fn push_button(self) -> bool {
    self.bit(17)
  }
  pub(crate) fn combo(self) -> bool {
    self.bit(18)
  }
  pub(crate) fn edit(self) -> bool {
    self.bit(19)
  }
  pub(crate) fn multi_select(self) -> bool {
    self.bit(22)
  }
  pub(crate) fn comb(self) -> bool {
    self.bit(25)
  }
  pub(crate) fn rich_text(self) -> bool {
    self.bit(26)
  }
}

/// `/FT`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FieldType {
  Text,
  Button,
  Choice,
  Signature,
}

impl FieldType {
  fn from_name(name: &[u8]) -> Option<Self> {
    match name {
      b"Tx" => Some(Self::Text),
      b"Btn" => Some(Self::Button),
      b"Ch" => Some(Self::Choice),
      b"Sig" => Some(Self::Signature),
      _ => None,
    }
  }
}

/// One dictionary of the field tree: a field, a widget, or a merged field+widget.
#[derive(Debug)]
pub(crate) struct FieldNode {
  pub object_id: Option<ObjectId>,
  /// Position in the parent's `/Kids` (or the root `/Fields`) when the dictionary is inline.
  pub inline_index: Option<usize>,
  pub is_root: bool,
  pub partial_name: String,
  pub field_type: Option<FieldType>,
  pub flags: Option<u32>,
  pub value: Option<Object>,
  pub default_value: Option<Object>,
  pub max_len: Option<u32>,
  pub options: Option<Vec<ChoiceOption>>,
  pub tooltip: Option<String>,
  pub mapping_name: Option<String>,
  pub has_actions: bool,
  pub parent: Option<usize>,
  pub children: Vec<usize>,
}

/// Arena of field nodes plus the diagnostics gathered while walking.
#[derive(Debug, Default)]
pub(crate) struct FieldTree {
  pub nodes: Vec<FieldNode>,
  pub warnings: Vec<String>,
  pub truncated: bool,
}

impl FieldTree {
  /// Walk the `/Fields` array through `/Kids`, dereferencing every entry, deduplicating shared
  /// or cyclic nodes, and capping depth at [`MAX_FIELD_DEPTH`].
  pub(crate) fn walk(doc: &Document, fields: &[Object]) -> FieldTree {
    let mut tree = FieldTree::default();
    let mut visited = HashSet::new();
    for (index, entry) in fields.iter().enumerate() {
      tree.visit(doc, entry, None, index, 0, &mut visited);
    }
    tree
  }

  fn visit(
    &mut self,
    doc: &Document,
    entry: &Object,
    parent: Option<usize>,
    index: usize,
    depth: usize,
    visited: &mut HashSet<ObjectId>,
  ) {
    if depth >= MAX_FIELD_DEPTH {
      self.truncated = true;
      let under = parent.map(|p| self.full_name(p)).unwrap_or_default();
      self.warnings.push(format!(
        "field tree depth cap ({MAX_FIELD_DEPTH}) reached under '{under}'"
      ));
      return;
    }
    let (object_id, dict): (Option<ObjectId>, &Dictionary) = match entry {
      Object::Reference(id) => match doc.get_dictionary(*id) {
        Ok(d) => {
          if !visited.insert(*id) {
            return;
          }
          (Some(*id), d)
        }
        Err(_) => {
          self
            .warnings
            .push(format!("/Kids: unresolved reference {} {} R", id.0, id.1));
          return;
        }
      },
      Object::Dictionary(d) => (None, d),
      other => {
        self.warnings.push(format!(
          "/Kids: entry is not a dictionary ({})",
          other.enum_variant()
        ));
        return;
      }
    };

    let node = self.node_from(doc, dict, object_id, parent, index);
    let idx = self.nodes.len();
    self.nodes.push(node);
    if let Some(p) = parent {
      self.nodes[p].children.push(idx);
    }

    if let Ok(kids_obj) = dict.get(b"Kids") {
      match deref(doc, kids_obj, &mut self.warnings, "/Kids") {
        Some(Object::Array(kids)) => {
          for (kid_index, kid) in kids.iter().enumerate() {
            self.visit(doc, kid, Some(idx), kid_index, depth + 1, visited);
          }
        }
        Some(other) => self.warnings.push(format!(
          "/Kids of '{}' is not an array ({})",
          self.full_name(idx),
          other.enum_variant()
        )),
        None => {}
      }
    }
  }

  fn node_from(
    &mut self,
    doc: &Document,
    dict: &Dictionary,
    object_id: Option<ObjectId>,
    parent: Option<usize>,
    index: usize,
  ) -> FieldNode {
    let warnings = &mut self.warnings;
    let get = |key: &[u8]| dict.get(key).ok();
    let text = |key: &[u8], warnings: &mut Vec<String>| {
      get(key).and_then(|o| decode_text_object(o, doc, warnings))
    };
    let int = |key: &[u8], warnings: &mut Vec<String>| {
      get(key)
        .and_then(|o| deref(doc, o, warnings, "field entry"))
        .and_then(|o| match o {
          Object::Integer(i) => u32::try_from(*i).ok(),
          Object::Real(r) if *r >= 0.0 => Some(*r as u32),
          _ => None,
        })
    };

    let partial_name = text(b"T", warnings).unwrap_or_default();
    let field_type = get(b"FT")
      .and_then(|o| deref(doc, o, warnings, "/FT"))
      .and_then(|o| match o {
        Object::Name(n) => FieldType::from_name(n),
        _ => None,
      });
    let flags = int(b"Ff", warnings);
    let max_len = int(b"MaxLen", warnings);
    let options = get(b"Opt").map(|o| parse_options(doc, o, warnings));
    let tooltip = text(b"TU", warnings);
    let mapping_name = text(b"TM", warnings);

    FieldNode {
      object_id,
      inline_index: object_id.is_none().then_some(index),
      is_root: parent.is_none(),
      partial_name,
      field_type,
      flags,
      value: get(b"V").cloned(),
      default_value: get(b"DV").cloned(),
      max_len,
      options,
      tooltip,
      mapping_name,
      has_actions: dict.has(b"AA") || dict.has(b"A"),
      parent,
      children: Vec::new(),
    }
  }

  /// First `Some` walking from `i` up the parent chain.
  fn effective<'s, T>(&'s self, i: usize, pick: impl Fn(&'s FieldNode) -> Option<T>) -> Option<T> {
    let mut cur = Some(i);
    while let Some(c) = cur {
      let node = &self.nodes[c];
      if let Some(v) = pick(node) {
        return Some(v);
      }
      cur = node.parent;
    }
    None
  }

  pub(crate) fn effective_type(&self, i: usize) -> Option<FieldType> {
    self.effective(i, |n| n.field_type)
  }

  pub(crate) fn effective_flags(&self, i: usize) -> FieldFlags {
    FieldFlags(self.effective(i, |n| n.flags).unwrap_or(0))
  }

  pub(crate) fn effective_value(&self, i: usize) -> Option<&Object> {
    self.effective(i, |n| n.value.as_ref())
  }

  pub(crate) fn effective_default(&self, i: usize) -> Option<&Object> {
    self.effective(i, |n| n.default_value.as_ref())
  }

  pub(crate) fn effective_max_len(&self, i: usize) -> Option<u32> {
    self.effective(i, |n| n.max_len)
  }

  pub(crate) fn effective_options(&self, i: usize) -> Option<&[ChoiceOption]> {
    self.effective(i, |n| n.options.as_deref())
  }

  pub(crate) fn effective_tooltip(&self, i: usize) -> Option<&str> {
    self.effective(i, |n| n.tooltip.as_deref())
  }

  /// Fully qualified name: non-empty partial names from the root down, joined with `.`.
  pub(crate) fn full_name(&self, i: usize) -> String {
    let mut parts = Vec::new();
    let mut cur = Some(i);
    while let Some(c) = cur {
      let node = &self.nodes[c];
      if !node.partial_name.is_empty() {
        parts.push(node.partial_name.as_str());
      }
      cur = node.parent;
    }
    parts.reverse();
    parts.join(".")
  }

  /// An unnamed node with a parent is a widget of that parent, never a field of its own.
  fn is_widget_only(&self, i: usize) -> bool {
    let node = &self.nodes[i];
    node.partial_name.is_empty() && !node.is_root
  }

  /// A logical field: has an effective `/FT`, no child carrying a non-empty `/T`, and is
  /// either named or a root `/Fields` entry.
  pub(crate) fn is_logical(&self, i: usize) -> bool {
    if self.effective_type(i).is_none() || self.is_widget_only(i) {
      return false;
    }
    !self.nodes[i]
      .children
      .iter()
      .any(|&c| !self.nodes[c].partial_name.is_empty())
  }

  /// Identity of the logical field that node `i` belongs to: the field dictionary's object id,
  /// or a synthesized `"<parent id>/kid I"` / `"/Fields I"` when that dictionary is inline.
  pub(crate) fn field_object_id(&self, i: usize) -> String {
    let node = &self.nodes[i];
    if self.is_widget_only(i) {
      return self.field_object_id(node.parent.expect("widget-only nodes have a parent"));
    }
    match (node.object_id, node.parent) {
      (Some(id), _) => object_id_string(id),
      (None, Some(p)) => format!(
        "{}/kid {}",
        self.field_object_id(p),
        node.inline_index.unwrap_or(0)
      ),
      (None, None) => format!("/Fields {}", node.inline_index.unwrap_or(0)),
    }
  }
}

/// `/Opt`: each entry is a text string (export = display) or an `[export display]` pair.
pub(crate) fn parse_options(
  doc: &Document,
  obj: &Object,
  warnings: &mut Vec<String>,
) -> Vec<ChoiceOption> {
  let Some(Object::Array(items)) = deref(doc, obj, warnings, "/Opt") else {
    return Vec::new();
  };
  items
    .iter()
    .filter_map(|item| match deref(doc, item, warnings, "/Opt entry")? {
      Object::Array(pair) if pair.len() >= 2 => Some(ChoiceOption {
        export_value: decode_text_object(&pair[0], doc, warnings).unwrap_or_default(),
        display_value: decode_text_object(&pair[1], doc, warnings).unwrap_or_default(),
      }),
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

/// `/Rect`: every numeric element in file order, non-numeric entries skipped (same convention
/// as `PageAnnotation.rect`).
pub(crate) fn parse_rect(obj: &Object) -> Vec<f64> {
  match obj {
    Object::Array(items) => items
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

// ── Widgets on pages ─────────────────────────────────────────────

/// A `/Subtype /Widget` annotation found in some page's `/Annots`.
#[derive(Debug, Clone)]
pub(crate) struct PlacedWidget {
  /// `"N G obj"`, or `"page P annot I"` for a dictionary written inline in `/Annots`.
  pub object_id: String,
  /// `None` for an inline dictionary (which can never be joined to a field by identity).
  pub key: Option<ObjectId>,
  /// 1-based page number.
  pub page: u32,
  /// Raw `/Rect` numbers in file order (same convention as `PageAnnotation.rect`).
  pub rect: Vec<f64>,
  pub dict: Dictionary,
}

/// Which placement of a logical field a widget is: the widget, its index and the field's
/// placement count. Needed by the button-state rules (a one-widget radio group behaves like a
/// checkbox; a state-less radio widget is reported by index).
#[derive(Clone, Copy)]
pub(crate) struct PlacementRef<'a> {
  pub widget: &'a PlacedWidget,
  pub index: u32,
  pub count: u32,
}

/// Every widget annotation on every page, in page order. `/Annots` may be a direct array or a
/// reference to one; entries may be references or inline dictionaries. Object ids are kept so
/// widgets can be joined to field nodes by identity.
pub(crate) fn scan_widgets(doc: &Document, warnings: &mut Vec<String>) -> Vec<PlacedWidget> {
  let mut widgets = Vec::new();
  for (page_num, page_id) in doc.get_pages() {
    let Ok(page) = doc.get_dictionary(page_id) else {
      continue;
    };
    let Ok(annots_obj) = page.get(b"Annots") else {
      continue;
    };
    let entries = match deref(
      doc,
      annots_obj,
      warnings,
      &format!("page {page_num}: /Annots"),
    ) {
      Some(Object::Array(entries)) => entries,
      Some(other) => {
        warnings.push(format!(
          "page {page_num}: /Annots is not an array ({})",
          other.enum_variant()
        ));
        continue;
      }
      None => continue,
    };
    for (index, entry) in entries.iter().enumerate() {
      let (key, dict): (Option<ObjectId>, &Dictionary) = match entry {
        Object::Reference(id) => match doc.get_dictionary(*id) {
          Ok(d) => (Some(*id), d),
          Err(_) => {
            warnings.push(format!(
              "page {page_num}: /Annots[{index}] unresolved reference {} {} R",
              id.0, id.1
            ));
            continue;
          }
        },
        Object::Dictionary(d) => (None, d),
        other => {
          warnings.push(format!(
            "page {page_num}: /Annots[{index}] is not a dictionary ({})",
            other.enum_variant()
          ));
          continue;
        }
      };
      let is_widget = dict
        .get(b"Subtype")
        .ok()
        .and_then(|o| deref(doc, o, warnings, "/Subtype"))
        .is_some_and(|o| matches!(o, Object::Name(n) if n == b"Widget"));
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

// ── Button state helpers ─────────────────────────────────────────

/// The widget's appearance "on" state: the first key of `/AP /N` that is not `Off`. `None`
/// when the widget has no appearance dictionary; never a fabricated default.
pub(crate) fn on_state_of(
  dict: &Dictionary,
  doc: &Document,
  warnings: &mut Vec<String>,
) -> Option<String> {
  let ap = match deref(doc, dict.get(b"AP").ok()?, warnings, "/AP")? {
    Object::Dictionary(d) => d,
    _ => return None,
  };
  let n = match deref(doc, ap.get(b"N").ok()?, warnings, "/AP /N")? {
    Object::Dictionary(d) => d,
    _ => return None,
  };
  n.iter()
    .map(|(k, _)| k.as_slice())
    .find(|k| *k != b"Off")
    .map(decode_name_bytes)
}

/// Map an appearance state name onto the field's `/Opt` export value when the state is an index
/// into `/Opt`; otherwise the state name is its own export value (ISO 32000-1 §12.7.4.2.3).
pub(crate) fn export_value_for(state: &str, options: Option<&[ChoiceOption]>) -> String {
  if let Some(opts) = options
    && let Ok(idx) = state.parse::<usize>()
    && let Some(opt) = opts.get(idx)
  {
    return opt.export_value.clone();
  }
  state.to_string()
}

/// A button's `/V` as a state name (names are the norm; a text string is accepted too).
fn button_value_name(
  doc: &Document,
  tree: &FieldTree,
  i: usize,
  warnings: &mut Vec<String>,
) -> Option<String> {
  let v = deref(doc, tree.effective_value(i)?, warnings, "/V")?;
  match v {
    Object::Name(n) => Some(decode_name_bytes(n)),
    Object::String(..) => decode_text_object(v, doc, warnings),
    _ => None,
  }
}

/// The widget's `/AS`, decoded.
fn appearance_state_of(
  dict: &Dictionary,
  doc: &Document,
  warnings: &mut Vec<String>,
) -> Option<String> {
  match deref(doc, dict.get(b"AS").ok()?, warnings, "/AS")? {
    Object::Name(n) => Some(decode_name_bytes(n)),
    _ => None,
  }
}

/// Resolve `checked` / `selected` for one button widget. Rules, in order: `/AS` when present;
/// else `/V` equals this widget's state name; else, with no appearance states at all, a checkbox
/// (or a one-widget radio group) is on iff `/V` is a non-`Off` name, and a radio widget is
/// reported unselected with a warning, because a group value cannot identify a state-less widget.
fn button_is_on(
  doc: &Document,
  tree: &FieldTree,
  i: usize,
  placement: Option<PlacementRef<'_>>,
  on_state: Option<&str>,
  is_radio: bool,
  warnings: &mut Vec<String>,
) -> bool {
  let as_state = placement.and_then(|p| appearance_state_of(&p.widget.dict, doc, warnings));
  let v_name = button_value_name(doc, tree, i, warnings);
  match (as_state, on_state) {
    (Some(a), _) => a != "Off",
    (None, Some(s)) => v_name.as_deref() == Some(s),
    (None, None) => {
      let single = placement.is_some_and(|p| p.count == 1);
      if is_radio && !single {
        if let Some(p) = placement {
          warnings.push(format!(
            "radio '{}' widget {} has no appearance states; selection unknown",
            tree.full_name(i),
            p.index
          ));
        }
        false
      } else {
        v_name.as_deref().is_some_and(|v| v != "Off")
      }
    }
  }
}

// ── Kind and common-field resolution ─────────────────────────────

/// Kind-specific payload for logical field `i`, seen through one placement (or none, for an
/// unplaced field).
pub(crate) fn resolve_kind(
  doc: &Document,
  tree: &FieldTree,
  i: usize,
  placement: Option<PlacementRef<'_>>,
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
      let is_radio = flags.radio();
      let on_state = placement.and_then(|p| on_state_of(&p.widget.dict, doc, warnings));
      let export_value = on_state
        .as_deref()
        .map(|s| export_value_for(s, tree.effective_options(i)));
      let on = button_is_on(
        doc,
        tree,
        i,
        placement,
        on_state.as_deref(),
        is_radio,
        warnings,
      );
      if is_radio {
        FieldKind::Radio {
          on_state,
          export_value,
          selected: on,
        }
      } else {
        FieldKind::Checkbox {
          on_state,
          export_value,
          checked: on,
        }
      }
    }
    Some(FieldType::Choice) => {
      let options = tree
        .effective_options(i)
        .map(<[_]>::to_vec)
        .unwrap_or_default();
      if flags.combo() {
        FieldKind::ComboBox {
          options,
          editable: flags.edit(),
        }
      } else {
        let values = tree
          .effective_value(i)
          .and_then(|v| deref(doc, v, warnings, "/V"))
          .and_then(|v| match v {
            Object::Array(items) => Some(
              items
                .iter()
                .filter_map(|item| decode_text_object(item, doc, warnings))
                .collect::<Vec<String>>(),
            ),
            _ => None,
          });
        FieldKind::ListBox {
          options,
          multi_select: flags.multi_select(),
          values,
        }
      }
    }
    Some(FieldType::Signature) => FieldKind::Signature {
      signed: tree
        .effective_value(i)
        .and_then(|v| deref(doc, v, warnings, "/V"))
        .is_some_and(|v| matches!(v, Object::Dictionary(_))),
    },
    None => FieldKind::Unknown {},
  }
}

/// The value of field `i` as reported to consumers: buttons in export-value form, signatures
/// never (their `/V` is signature data), everything else as decoded text.
fn field_value(
  doc: &Document,
  tree: &FieldTree,
  i: usize,
  raw: Option<&Object>,
  warnings: &mut Vec<String>,
) -> Option<String> {
  match tree.effective_type(i) {
    Some(FieldType::Signature) => None,
    Some(FieldType::Button) => {
      let v = deref(doc, raw?, warnings, "/V")?;
      let state = match v {
        Object::Name(n) => Some(decode_name_bytes(n)),
        Object::String(..) => decode_text_object(v, doc, warnings),
        _ => None,
      }?;
      Some(export_value_for(&state, tree.effective_options(i)))
    }
    _ => decode_text_object(raw?, doc, warnings),
  }
}

/// The common fields of one placement of logical field `i`.
pub(crate) fn common_for(
  doc: &Document,
  tree: &FieldTree,
  i: usize,
  placement: PlacementRef<'_>,
  warnings: &mut Vec<String>,
) -> FieldCommon {
  let flags = tree.effective_flags(i);
  let node = &tree.nodes[i];
  let widget = placement.widget;
  FieldCommon {
    page: widget.page,
    rect: widget.rect.clone(),
    object_id: widget.object_id.clone(),
    field_object_id: tree.field_object_id(i),
    name: tree.full_name(i),
    partial_name: node.partial_name.clone(),
    widget_index: placement.index,
    widget_count: placement.count,
    value: field_value(doc, tree, i, tree.effective_value(i), warnings),
    default_value: field_value(doc, tree, i, tree.effective_default(i), warnings),
    tooltip: tree.effective_tooltip(i).map(str::to_owned),
    mapping_name: node.mapping_name.clone(),
    read_only: flags.read_only(),
    required: flags.required(),
    no_export: flags.no_export(),
    has_actions: node.has_actions || widget.dict.has(b"AA") || widget.dict.has(b"A"),
  }
}

// ── Extraction entry point ───────────────────────────────────────

/// Everything the forms module produces for one document.
pub(crate) struct FormsExtraction {
  /// Present when `/AcroForm` exists or any widget was found.
  pub info: Option<FormInfo>,
  /// One entry per placed widget, sorted (see [`sort_fields`]).
  pub fields: Vec<PageFormField>,
}

/// Read-only per-page form field extraction (spec §5.2). Never writes to `doc`.
pub(crate) fn extract_forms(doc: &Document) -> FormsExtraction {
  let mut warnings = Vec::new();

  // 1. Widgets on pages, independent of /AcroForm.
  let widgets = scan_widgets(doc, &mut warnings);
  let by_key: HashMap<ObjectId, usize> = widgets
    .iter()
    .enumerate()
    .filter_map(|(k, w)| w.key.map(|id| (id, k)))
    .collect();
  let mut claimed = vec![false; widgets.len()];

  // 2. /AcroForm, if present.
  let acroform = doc
    .catalog()
    .ok()
    .and_then(|cat| cat.get(b"AcroForm").ok())
    .and_then(|o| deref(doc, o, &mut warnings, "/AcroForm"))
    .and_then(|o| match o {
      Object::Dictionary(d) => Some(d),
      _ => None,
    });
  let mut need_appearances = false;
  let mut sig_flags = 0u32;
  let mut has_xfa = false;
  let mut field_entries: Vec<Object> = Vec::new();
  if let Some(af) = acroform {
    need_appearances = af
      .get(b"NeedAppearances")
      .ok()
      .and_then(|o| deref(doc, o, &mut warnings, "/NeedAppearances"))
      .is_some_and(|o| matches!(o, Object::Boolean(true)));
    sig_flags = af
      .get(b"SigFlags")
      .ok()
      .and_then(|o| deref(doc, o, &mut warnings, "/SigFlags"))
      .and_then(|o| match o {
        Object::Integer(i) => u32::try_from(*i).ok(),
        _ => None,
      })
      .unwrap_or(0);
    has_xfa = af.has(b"XFA");
    match af
      .get(b"Fields")
      .ok()
      .and_then(|o| deref(doc, o, &mut warnings, "/Fields"))
    {
      Some(Object::Array(items)) => field_entries = items.clone(),
      Some(other) => warnings.push(format!(
        "/AcroForm /Fields is not an array ({})",
        other.enum_variant()
      )),
      None => {}
    }
  }

  // 3. Field tree → placements and unplaced fields.
  let tree = FieldTree::walk(doc, &field_entries);
  let mut fields = Vec::new();
  let mut unplaced = Vec::new();
  let mut field_count = 0u32;
  for i in 0..tree.nodes.len() {
    if !tree.is_logical(i) {
      continue;
    }
    field_count += 1;
    let node = &tree.nodes[i];
    let mut placements: Vec<usize> = Vec::new();
    if let Some(k) = node.object_id.and_then(|id| by_key.get(&id)) {
      placements.push(*k);
    }
    for &c in &node.children {
      let kid = &tree.nodes[c];
      if kid.partial_name.is_empty()
        && let Some(k) = kid.object_id.and_then(|id| by_key.get(&id))
      {
        placements.push(*k);
      }
    }
    if placements.is_empty() {
      unplaced.push(unplaced_for(doc, &tree, i, &mut warnings));
      continue;
    }
    let count = placements.len() as u32;
    for (index, &k) in placements.iter().enumerate() {
      claimed[k] = true;
      let placement = PlacementRef {
        widget: &widgets[k],
        index: index as u32,
        count,
      };
      let kind = resolve_kind(doc, &tree, i, Some(placement), &mut warnings);
      let common = common_for(doc, &tree, i, placement, &mut warnings);
      fields.push(PageFormField::new(common, kind));
    }
  }

  // 4. Widgets no field claims.
  for (k, widget) in widgets.iter().enumerate() {
    if !claimed[k] {
      fields.push(unknown_for(doc, widget, &mut warnings));
    }
  }

  // 5. Order and summarise.
  sort_fields(&mut fields);
  let mut all_warnings = tree.warnings.clone();
  all_warnings.extend(warnings);
  let info = (acroform.is_some() || !fields.is_empty()).then(|| FormInfo {
    has_acro_form: acroform.is_some(),
    has_xfa,
    need_appearances,
    sig_flags,
    field_count,
    widget_count: fields.len() as u32,
    unplaced_fields: unplaced,
    truncated: tree.truncated,
    warnings: all_warnings,
  });
  FormsExtraction { info, fields }
}

/// A logical field with no placed widget: full payload, no geometry.
fn unplaced_for(
  doc: &Document,
  tree: &FieldTree,
  i: usize,
  warnings: &mut Vec<String>,
) -> UnplacedFormField {
  let kind = resolve_kind(doc, tree, i, None, warnings);
  let flags = tree.effective_flags(i);
  let node = &tree.nodes[i];
  let mut u = UnplacedFormField {
    field_object_id: tree.field_object_id(i),
    name: tree.full_name(i),
    partial_name: node.partial_name.clone(),
    kind: kind.kind_name().to_string(),
    value: field_value(doc, tree, i, tree.effective_value(i), warnings),
    values: None,
    default_value: field_value(doc, tree, i, tree.effective_default(i), warnings),
    tooltip: tree.effective_tooltip(i).map(str::to_owned),
    mapping_name: node.mapping_name.clone(),
    options: None,
    max_len: None,
    read_only: flags.read_only(),
    required: flags.required(),
    no_export: flags.no_export(),
    has_actions: node.has_actions,
    multiline: false,
    password: false,
    comb: false,
    rich_text: false,
    editable: false,
    multi_select: false,
    signed: false,
  };
  match kind {
    FieldKind::Text {
      multiline,
      password,
      comb,
      rich_text,
      max_len,
    } => {
      u.multiline = multiline;
      u.password = password;
      u.comb = comb;
      u.rich_text = rich_text;
      u.max_len = max_len;
    }
    FieldKind::ComboBox { options, editable } => {
      u.options = Some(options);
      u.editable = editable;
    }
    FieldKind::ListBox {
      options,
      multi_select,
      values,
    } => {
      u.options = Some(options);
      u.multi_select = multi_select;
      u.values = values;
    }
    FieldKind::Signature { signed } => u.signed = signed,
    FieldKind::Checkbox { .. }
    | FieldKind::Radio { .. }
    | FieldKind::PushButton {}
    | FieldKind::Unknown {} => {}
  }
  u
}

/// A widget annotation no field claims: whatever its own dictionary says, typed `Unknown`.
fn unknown_for(doc: &Document, widget: &PlacedWidget, warnings: &mut Vec<String>) -> PageFormField {
  let dict = &widget.dict;
  let text = |key: &[u8], warnings: &mut Vec<String>| {
    dict
      .get(key)
      .ok()
      .and_then(|o| decode_text_object(o, doc, warnings))
  };
  let name = text(b"T", warnings).unwrap_or_default();
  let flags = FieldFlags(
    dict
      .get(b"Ff")
      .ok()
      .and_then(|o| deref(doc, o, warnings, "/Ff"))
      .and_then(|o| match o {
        Object::Integer(i) => u32::try_from(*i).ok(),
        _ => None,
      })
      .unwrap_or(0),
  );
  let common = FieldCommon {
    page: widget.page,
    rect: widget.rect.clone(),
    object_id: widget.object_id.clone(),
    field_object_id: widget.object_id.clone(),
    partial_name: name.clone(),
    name,
    widget_index: 0,
    widget_count: 1,
    value: text(b"V", warnings),
    default_value: text(b"DV", warnings),
    tooltip: text(b"TU", warnings),
    mapping_name: text(b"TM", warnings),
    read_only: flags.read_only(),
    required: flags.required(),
    no_export: flags.no_export(),
    has_actions: dict.has(b"AA") || dict.has(b"A"),
  };
  PageFormField::new(common, FieldKind::Unknown {})
}

/// Page ascending, then top edge descending, then left edge ascending, then name, then object id.
/// A missing coordinate counts as `0.0`.
pub(crate) fn sort_fields(fields: &mut [PageFormField]) {
  fn top(r: &[f64]) -> f64 {
    r.get(1)
      .copied()
      .unwrap_or(0.0)
      .max(r.get(3).copied().unwrap_or(0.0))
  }
  fn left(r: &[f64]) -> f64 {
    r.first()
      .copied()
      .unwrap_or(0.0)
      .min(r.get(2).copied().unwrap_or(0.0))
  }
  fields.sort_by(|a, b| {
    a.page()
      .cmp(&b.page())
      .then_with(|| top(b.rect()).total_cmp(&top(a.rect())))
      .then_with(|| left(a.rect()).total_cmp(&left(b.rect())))
      .then_with(|| a.name().cmp(b.name()))
      .then_with(|| a.object_id().cmp(b.object_id()))
  });
}

/// Sorted unique page numbers of the given placements (mirrors `imagePages`).
pub(crate) fn form_field_pages(fields: &[PageFormField]) -> Vec<u32> {
  fields
    .iter()
    .map(PageFormField::page)
    .collect::<BTreeSet<u32>>()
    .into_iter()
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;
  use lopdf::{StringFormat, dictionary};

  fn doc(objects: Vec<(ObjectId, Object)>) -> Document {
    let mut d = Document::with_version("1.7");
    for (id, obj) in objects {
      d.max_id = d.max_id.max(id.0);
      d.objects.insert(id, obj);
    }
    d
  }

  fn name(n: &str) -> Object {
    Object::Name(n.as_bytes().to_vec())
  }

  fn refs(ids: &[(u32, u16)]) -> Object {
    Object::Array(ids.iter().map(|&id| Object::Reference(id)).collect())
  }

  fn rect() -> Object {
    Object::Array(vec![
      Object::Integer(0),
      Object::Integer(0),
      Object::Integer(10),
      Object::Integer(10),
    ])
  }

  fn walk_root(d: &Document, root: ObjectId) -> FieldTree {
    FieldTree::walk(d, &[Object::Reference(root)])
  }

  fn text(tree: &FieldTree, d: &Document, i: usize) -> Option<String> {
    let mut w = vec![];
    tree
      .effective_value(i)
      .and_then(|v| decode_text_object(v, d, &mut w))
  }

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
    let d = doc(vec![]);
    let mut w = vec![];
    assert_eq!(
      decode_text_object(
        &Object::String(
          vec![0xFE, 0xFF, 0x00, 0x4D, 0x00, 0xFC],
          StringFormat::Hexadecimal
        ),
        &d,
        &mut w
      ),
      Some("Mü".into())
    );
    assert_eq!(
      decode_text_object(
        &Object::String(b"Stra\xDFe".to_vec(), StringFormat::Literal),
        &d,
        &mut w
      ),
      Some("Straße".into())
    );
    assert_eq!(
      decode_text_object(&Object::Integer(42), &d, &mut w),
      Some("42".into())
    );
    assert_eq!(
      decode_text_object(&Object::Dictionary(Dictionary::new()), &d, &mut w),
      None
    );
    assert!(w.is_empty());
  }

  #[test]
  fn indirect_values_resolve_and_dangling_refs_warn() {
    let d = doc(vec![
      ((9, 0), Object::string_literal("via ref")),
      (
        (10, 0),
        Object::Array(vec![
          Object::string_literal("a"),
          Object::string_literal("c"),
        ]),
      ),
    ]);
    let mut w = vec![];
    assert_eq!(
      decode_text_object(&Object::Reference((9, 0)), &d, &mut w),
      Some("via ref".into())
    );
    assert_eq!(
      decode_text_object(&Object::Reference((10, 0)), &d, &mut w),
      Some("a".into())
    );
    assert_eq!(
      decode_text_object(&Object::Reference((99, 0)), &d, &mut w),
      None
    );
    assert_eq!(w.len(), 1);
    assert!(w[0].contains("99 0 R"));
  }

  #[test]
  fn inheritance_resolves_type_flags_value_through_parent() {
    let d = doc(vec![
      (
        (1, 0),
        dictionary! {
          "T" => Object::string_literal("parent"),
          "FT" => name("Tx"),
          "Ff" => Object::Integer(1),
          "V" => Object::string_literal("shared"),
          "Kids" => refs(&[(2, 0)])
        }
        .into(),
      ),
      (
        (2, 0),
        dictionary! {
          "T" => Object::string_literal("kid"),
          "Parent" => Object::Reference((1, 0))
        }
        .into(),
      ),
    ]);
    let tree = walk_root(&d, (1, 0));
    assert_eq!(tree.nodes.len(), 2);
    assert_eq!(tree.effective_type(1), Some(FieldType::Text));
    assert!(tree.effective_flags(1).read_only());
    assert_eq!(text(&tree, &d, 1), Some("shared".into()));
    assert_eq!(tree.full_name(1), "parent.kid");
  }

  #[test]
  fn container_with_named_child_is_not_logical_but_child_is() {
    let d = doc(vec![
      (
        (1, 0),
        dictionary! {
          "T" => Object::string_literal("parent"),
          "FT" => name("Tx"),
          "Kids" => refs(&[(2, 0)])
        }
        .into(),
      ),
      (
        (2, 0),
        dictionary! {
          "T" => Object::string_literal("kid"),
          "Rect" => rect(),
          "Parent" => Object::Reference((1, 0))
        }
        .into(),
      ),
    ]);
    let tree = walk_root(&d, (1, 0));
    assert!(!tree.is_logical(0));
    assert!(tree.is_logical(1));
  }

  #[test]
  fn unnamed_kids_make_parent_logical() {
    let d = doc(vec![
      (
        (1, 0),
        dictionary! {
          "T" => Object::string_literal("color"),
          "FT" => name("Btn"),
          "Ff" => Object::Integer(1 << 15),
          "Kids" => refs(&[(2, 0), (3, 0)])
        }
        .into(),
      ),
      (
        (2, 0),
        dictionary! { "Rect" => rect(), "Parent" => Object::Reference((1, 0)) }.into(),
      ),
      (
        (3, 0),
        dictionary! { "Rect" => rect(), "Parent" => Object::Reference((1, 0)) }.into(),
      ),
    ]);
    let tree = walk_root(&d, (1, 0));
    assert!(tree.is_logical(0));
    assert!(!tree.is_logical(1));
    assert!(!tree.is_logical(2));
    assert_eq!(tree.effective_type(1), Some(FieldType::Button));
  }

  #[test]
  fn unnamed_root_field_is_still_logical() {
    let d = doc(vec![(
      (1, 0),
      dictionary! { "FT" => name("Tx"), "Rect" => rect() }.into(),
    )]);
    let tree = walk_root(&d, (1, 0));
    assert!(tree.nodes[0].is_root);
    assert!(tree.is_logical(0));
    assert_eq!(tree.full_name(0), "");
  }

  #[test]
  fn inline_unnamed_kid_is_a_widget_of_its_parent() {
    let d = doc(vec![(
      (5, 0),
      dictionary! {
        "T" => Object::string_literal("f"),
        "FT" => name("Tx"),
        "Kids" => Object::Array(vec![dictionary! { "Rect" => rect() }.into()])
      }
      .into(),
    )]);
    let tree = walk_root(&d, (5, 0));
    assert_eq!(tree.nodes.len(), 2);
    assert_eq!(tree.nodes[1].inline_index, Some(0));
    assert!(!tree.is_logical(1));
    assert_eq!(tree.field_object_id(1), "5 0 obj");
  }

  #[test]
  fn inline_named_field_gets_synthetic_id() {
    let d = doc(vec![(
      (5, 0),
      dictionary! {
        "T" => Object::string_literal("f"),
        "Kids" => Object::Array(vec![
          dictionary! { "T" => Object::string_literal("sub"), "FT" => name("Tx") }.into()
        ])
      }
      .into(),
    )]);
    let tree = walk_root(&d, (5, 0));
    assert!(tree.is_logical(1));
    assert_eq!(tree.full_name(1), "f.sub");
    assert_eq!(tree.field_object_id(1), "5 0 obj/kid 0");

    let root_inline = FieldTree::walk(
      &d,
      &[dictionary! { "T" => Object::string_literal("r"), "FT" => name("Tx") }.into()],
    );
    assert_eq!(root_inline.field_object_id(0), "/Fields 0");
  }

  #[test]
  fn kids_cycle_terminates_and_dedupes() {
    let d = doc(vec![
      (
        (1, 0),
        dictionary! {
          "T" => Object::string_literal("A"),
          "FT" => name("Tx"),
          "Kids" => refs(&[(2, 0)])
        }
        .into(),
      ),
      (
        (2, 0),
        dictionary! { "T" => Object::string_literal("B"), "Kids" => refs(&[(1, 0)]) }.into(),
      ),
    ]);
    let tree = walk_root(&d, (1, 0));
    assert_eq!(tree.nodes.len(), 2);
    assert!(tree.nodes[1].children.is_empty());
  }

  #[test]
  fn depth_is_capped_and_reported() {
    let mut objects = vec![];
    for i in 1..=150u32 {
      let mut dict = dictionary! { "T" => Object::string_literal(format!("n{i}")) };
      if i < 150 {
        dict.set("Kids", refs(&[(i + 1, 0)]));
      } else {
        dict.set("FT", name("Tx"));
      }
      objects.push(((i, 0), dict.into()));
    }
    let d = doc(objects);
    let tree = walk_root(&d, (1, 0));
    assert!(tree.nodes.len() <= 100);
    assert!(tree.truncated);
    assert!(tree.warnings.iter().any(|w| w.contains("depth")));
  }

  #[test]
  fn bad_kids_entries_warn() {
    let d = doc(vec![(
      (1, 0),
      dictionary! {
        "T" => Object::string_literal("p"),
        "FT" => name("Tx"),
        "Kids" => Object::Array(vec![Object::Integer(42), Object::Reference((7, 0))])
      }
      .into(),
    )]);
    let tree = walk_root(&d, (1, 0));
    assert_eq!(tree.nodes.len(), 1);
    assert_eq!(tree.warnings.len(), 2);
    assert!(!tree.truncated);
  }

  #[test]
  fn options_accept_strings_and_pairs() {
    let d = doc(vec![]);
    let mut w = vec![];
    let opts = parse_options(
      &d,
      &Object::Array(vec![
        Object::string_literal("a"),
        Object::Array(vec![
          Object::string_literal("US"),
          Object::string_literal("United States"),
        ]),
      ]),
      &mut w,
    );
    assert_eq!(
      opts,
      vec![
        ChoiceOption {
          export_value: "a".into(),
          display_value: "a".into()
        },
        ChoiceOption {
          export_value: "US".into(),
          display_value: "United States".into()
        },
      ]
    );
    assert!(w.is_empty());
  }

  #[test]
  fn rect_keeps_numeric_entries_in_order() {
    assert_eq!(
      parse_rect(&Object::Array(vec![
        Object::Integer(0),
        Object::Name(b"x".to_vec()),
        Object::Real(100.5),
      ])),
      vec![0.0, 100.5]
    );
  }

  // ── Task 3: widget scan and kind resolution ──────────────────

  /// Catalog (1,0), pages (2,0), page k at (100+k, 0) with the given `/Annots` value.
  fn pdf_with_pages(annots: Vec<Object>, extras: Vec<(ObjectId, Object)>) -> Document {
    let mut objects = extras;
    let mut kids = vec![];
    for (k, a) in annots.into_iter().enumerate() {
      let id = (100 + k as u32, 0);
      kids.push(Object::Reference(id));
      objects.push((
        id,
        dictionary! {
          "Type" => name("Page"),
          "Parent" => Object::Reference((2, 0)),
          "MediaBox" => rect(),
          "Annots" => a
        }
        .into(),
      ));
    }
    let count = kids.len() as i64;
    objects.push((
      (2, 0),
      dictionary! { "Type" => name("Pages"), "Kids" => Object::Array(kids), "Count" => Object::Integer(count) }
        .into(),
    ));
    objects.push((
      (1, 0),
      dictionary! { "Type" => name("Catalog"), "Pages" => Object::Reference((2, 0)) }.into(),
    ));
    let mut d = doc(objects);
    d.trailer.set("Root", Object::Reference((1, 0)));
    d
  }

  fn widget_dict(extra: Dictionary) -> Dictionary {
    let mut w =
      dictionary! { "Type" => name("Annot"), "Subtype" => name("Widget"), "Rect" => rect() };
    w.extend(&extra);
    w
  }

  /// A merged field+widget placed as the only widget of its own logical field.
  fn placed(id: ObjectId, dict: &Dictionary) -> PlacedWidget {
    PlacedWidget {
      object_id: object_id_string(id),
      key: Some(id),
      page: 1,
      rect: vec![0.0, 0.0, 10.0, 10.0],
      dict: dict.clone(),
    }
  }

  fn ap_n(keys: &[&str]) -> Object {
    let mut n = Dictionary::new();
    for k in keys {
      n.set(k.as_bytes().to_vec(), Object::Reference((90, 0)));
    }
    dictionary! { "N" => Object::Dictionary(n) }.into()
  }

  /// Resolve a single merged field (1,0): returns (kind, common, warnings).
  fn resolve_single(
    field: Dictionary,
    extras: Vec<(ObjectId, Object)>,
  ) -> (FieldKind, FieldCommon, Vec<String>) {
    let mut objects = extras;
    objects.push(((1, 0), field.clone().into()));
    let d = doc(objects);
    let tree = walk_root(&d, (1, 0));
    let w = placed((1, 0), &field);
    let mut warnings = vec![];
    let kind = resolve_kind(
      &d,
      &tree,
      0,
      Some(PlacementRef {
        widget: &w,
        index: 0,
        count: 1,
      }),
      &mut warnings,
    );
    let common = common_for(
      &d,
      &tree,
      0,
      PlacementRef {
        widget: &w,
        index: 0,
        count: 1,
      },
      &mut warnings,
    );
    (kind, common, warnings)
  }

  #[test]
  fn scan_finds_widgets_behind_indirect_annots_and_inline_dicts() {
    let d = pdf_with_pages(
      vec![
        Object::Reference((10, 0)),
        Object::Array(vec![
          widget_dict(dictionary! { "T" => Object::string_literal("inline") }).into(),
        ]),
      ],
      vec![
        (
          (10, 0),
          Object::Array(vec![Object::Reference((5, 0)), Object::Reference((6, 0))]),
        ),
        (
          (5, 0),
          widget_dict(dictionary! { "T" => Object::string_literal("ghost") }).into(),
        ),
        (
          (6, 0),
          dictionary! { "Type" => name("Annot"), "Subtype" => name("Link"), "Rect" => rect() }
            .into(),
        ),
      ],
    );
    let mut w = vec![];
    let widgets = scan_widgets(&d, &mut w);
    assert_eq!(widgets.len(), 2);
    assert_eq!(
      widgets.iter().map(|x| x.page).collect::<Vec<_>>(),
      vec![1, 2]
    );
    assert_eq!(widgets[0].object_id, "5 0 obj");
    assert_eq!(widgets[0].key, Some((5, 0)));
    assert_eq!(widgets[1].object_id, "page 2 annot 0");
    assert!(widgets[1].key.is_none());
    assert!(w.is_empty());
  }

  #[test]
  fn on_state_is_first_non_off_ap_key() {
    let d = doc(vec![]);
    let mut w = vec![];
    let with_ja = dictionary! { "AP" => ap_n(&["Off", "Ja"]) };
    assert_eq!(on_state_of(&with_ja, &d, &mut w), Some("Ja".into()));
    let only_off = dictionary! { "AP" => ap_n(&["Off"]) };
    assert_eq!(on_state_of(&only_off, &d, &mut w), None);
    let no_ap = dictionary! { "Rect" => rect() };
    assert_eq!(on_state_of(&no_ap, &d, &mut w), None);
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

  fn checkbox(
    with_ap: bool,
    as_state: Option<&str>,
    v: Option<&str>,
  ) -> (FieldKind, FieldCommon, Vec<String>) {
    let mut f =
      widget_dict(dictionary! { "T" => Object::string_literal("agree"), "FT" => name("Btn") });
    if with_ap {
      f.set("AP", ap_n(&["Off", "Ja"]));
    }
    if let Some(a) = as_state {
      f.set("AS", name(a));
    }
    if let Some(v) = v {
      f.set("V", name(v));
    }
    resolve_single(f, vec![])
  }

  fn is_checked(kind: &FieldKind) -> bool {
    match kind {
      FieldKind::Checkbox { checked, .. } => *checked,
      other => panic!("expected Checkbox, got {}", kind_label(other)),
    }
  }

  fn kind_label(kind: &FieldKind) -> &'static str {
    match kind {
      FieldKind::Text { .. } => "Text",
      FieldKind::Checkbox { .. } => "Checkbox",
      FieldKind::Radio { .. } => "Radio",
      FieldKind::ComboBox { .. } => "ComboBox",
      FieldKind::ListBox { .. } => "ListBox",
      FieldKind::PushButton {} => "PushButton",
      FieldKind::Signature { .. } => "Signature",
      FieldKind::Unknown {} => "Unknown",
    }
  }

  #[test]
  fn checkbox_checked_from_as_then_v_and_disagreement_is_visible() {
    let (k, _, _) = checkbox(true, Some("Ja"), None);
    assert!(is_checked(&k));
    let (k, _, _) = checkbox(true, None, Some("Ja"));
    assert!(is_checked(&k));
    let (k, c, _) = checkbox(true, Some("Off"), Some("Ja"));
    assert!(!is_checked(&k));
    assert_eq!(c.value, Some("Ja".into()));
    match k {
      FieldKind::Checkbox {
        on_state,
        export_value,
        ..
      } => {
        assert_eq!(on_state.as_deref(), Some("Ja"));
        assert_eq!(export_value.as_deref(), Some("Ja"));
      }
      _ => unreachable!(),
    }
  }

  #[test]
  fn checkbox_without_appearance_has_no_state_names() {
    let (k, c, w) = checkbox(false, None, Some("Ja"));
    match &k {
      FieldKind::Checkbox {
        on_state,
        export_value,
        checked,
      } => {
        assert_eq!(*on_state, None);
        assert_eq!(*export_value, None);
        assert!(*checked);
      }
      _ => unreachable!(),
    }
    assert_eq!(c.value, Some("Ja".into()));
    assert!(w.is_empty());
    let (k, _, _) = checkbox(false, None, None);
    assert!(!is_checked(&k));
    let (k, _, _) = checkbox(false, None, Some("Off"));
    assert!(!is_checked(&k));
  }

  /// Radio group (1,0) with `kids` widget dictionaries at (2,0), (3,0), …; returns per-widget
  /// (kind, common) plus the warnings.
  fn radio_group(
    group_extra: Dictionary,
    kids: Vec<Dictionary>,
  ) -> (Vec<(FieldKind, FieldCommon)>, Vec<String>) {
    let kid_ids: Vec<ObjectId> = (0..kids.len()).map(|k| (2 + k as u32, 0)).collect();
    let mut group = dictionary! {
      "T" => Object::string_literal("color"),
      "FT" => name("Btn"),
      "Ff" => Object::Integer(1 << 15),
      "Kids" => refs(&kid_ids)
    };
    group.extend(&group_extra);
    let mut objects = vec![((1, 0), group.into())];
    for (id, kid) in kid_ids.iter().zip(&kids) {
      let mut kd = widget_dict(kid.clone());
      kd.set("Parent", Object::Reference((1, 0)));
      objects.push((*id, kd.into()));
    }
    let d = doc(objects);
    let tree = walk_root(&d, (1, 0));
    let count = kids.len() as u32;
    let mut warnings = vec![];
    let mut out = vec![];
    for (index, id) in kid_ids.iter().enumerate() {
      let dict = d.get_dictionary(*id).unwrap().clone();
      let w = PlacedWidget {
        object_id: object_id_string(*id),
        key: Some(*id),
        page: 1,
        rect: vec![],
        dict,
      };
      let p = PlacementRef {
        widget: &w,
        index: index as u32,
        count,
      };
      let kind = resolve_kind(&d, &tree, 0, Some(p), &mut warnings);
      let common = common_for(&d, &tree, 0, p, &mut warnings);
      out.push((kind, common));
    }
    (out, warnings)
  }

  fn radio_parts(kind: &FieldKind) -> (Option<String>, Option<String>, bool) {
    match kind {
      FieldKind::Radio {
        on_state,
        export_value,
        selected,
      } => (on_state.clone(), export_value.clone(), *selected),
      other => panic!("expected Radio, got {}", kind_label(other)),
    }
  }

  #[test]
  fn radio_without_appearance_states_is_unselected_with_warning() {
    let (out, w) = radio_group(
      dictionary! { "V" => name("1") },
      vec![Dictionary::new(), Dictionary::new()],
    );
    assert_eq!(
      out
        .iter()
        .map(|(k, _)| radio_parts(k).2)
        .collect::<Vec<_>>(),
      vec![false, false]
    );
    assert_eq!(
      w.iter().filter(|m| m.contains("appearance states")).count(),
      2
    );

    let (out, w) = radio_group(dictionary! { "V" => name("On") }, vec![Dictionary::new()]);
    assert!(radio_parts(&out[0].0).2);
    assert!(w.is_empty());
  }

  #[test]
  fn radio_keeps_state_name_and_export_value_apart() {
    let opt = Object::Array(vec![
      Object::string_literal("Red"),
      Object::string_literal("Green"),
      Object::string_literal("Blue"),
    ]);
    let kid = |state: &str, as_state: &str| {
      dictionary! { "AP" => ap_n(&["Off", state]), "AS" => name(as_state) }
    };
    let (out, w) = radio_group(
      dictionary! { "V" => name("1"), "Opt" => opt },
      vec![kid("0", "Off"), kid("1", "1"), kid("2", "Off")],
    );
    let parts: Vec<_> = out.iter().map(|(k, _)| radio_parts(k)).collect();
    assert_eq!(
      parts
        .iter()
        .map(|p| p.0.clone().unwrap())
        .collect::<Vec<_>>(),
      vec!["0", "1", "2"]
    );
    assert_eq!(
      parts
        .iter()
        .map(|p| p.1.clone().unwrap())
        .collect::<Vec<_>>(),
      vec!["Red", "Green", "Blue"]
    );
    assert_eq!(
      parts.iter().map(|p| p.2).collect::<Vec<_>>(),
      vec![false, true, false]
    );
    assert!(out.iter().all(|(_, c)| c.value.as_deref() == Some("Green")));
    assert!(w.is_empty());
  }

  #[test]
  fn choice_kinds_and_values() {
    let combo = widget_dict(dictionary! {
      "T" => Object::string_literal("country"),
      "FT" => name("Ch"),
      "Ff" => Object::Integer(1 << 17 | 1 << 18),
      "Opt" => Object::Array(vec![Object::string_literal("US")]),
      "V" => Object::string_literal("US")
    });
    let (k, c, _) = resolve_single(combo, vec![]);
    match k {
      FieldKind::ComboBox { options, editable } => {
        assert!(editable);
        assert_eq!(options.len(), 1);
      }
      other => panic!("expected ComboBox, got {}", kind_label(&other)),
    }
    assert_eq!(c.value, Some("US".into()));

    let list = widget_dict(dictionary! {
      "T" => Object::string_literal("tags"),
      "FT" => name("Ch"),
      "Ff" => Object::Integer(1 << 21),
      "Opt" => Object::Array(vec![Object::string_literal("a"), Object::string_literal("b"), Object::string_literal("c")]),
      "V" => Object::Array(vec![Object::string_literal("a"), Object::string_literal("c")])
    });
    let (k, c, _) = resolve_single(list, vec![]);
    match k {
      FieldKind::ListBox {
        multi_select,
        values,
        ..
      } => {
        assert!(multi_select);
        assert_eq!(values, Some(vec!["a".into(), "c".into()]));
      }
      other => panic!("expected ListBox, got {}", kind_label(&other)),
    }
    assert_eq!(c.value, Some("a".into()));
  }

  #[test]
  fn signature_signed_only_when_v_resolves_to_a_dictionary() {
    let sig = |v: Option<Object>| {
      let mut f =
        widget_dict(dictionary! { "T" => Object::string_literal("sig"), "FT" => name("Sig") });
      if let Some(v) = v {
        f.set("V", v);
      }
      f
    };
    let signed = |k: &FieldKind| match k {
      FieldKind::Signature { signed } => *signed,
      other => panic!("expected Signature, got {}", kind_label(other)),
    };
    let (k, c, w) = resolve_single(
      sig(Some(Object::Reference((9, 0)))),
      vec![((9, 0), dictionary! { "Type" => name("Sig") }.into())],
    );
    assert!(signed(&k));
    assert_eq!(c.value, None);
    assert!(w.is_empty());
    let (k, _, _) = resolve_single(
      sig(Some(Object::Reference((9, 0)))),
      vec![((9, 0), Object::string_literal("x"))],
    );
    assert!(!signed(&k));
    let (k, _, w) = resolve_single(sig(Some(Object::Reference((99, 0)))), vec![]);
    assert!(!signed(&k));
    assert_eq!(w.len(), 1);
    let (k, c, _) = resolve_single(sig(None), vec![]);
    assert!(!signed(&k));
    assert_eq!(c.value, None);
  }

  #[test]
  fn choice_values_resolve_indirect_arrays() {
    let list = widget_dict(dictionary! {
      "T" => Object::string_literal("tags"),
      "FT" => name("Ch"),
      "Ff" => Object::Integer(1 << 21),
      "Opt" => Object::Reference((11, 0)),
      "V" => Object::Reference((10, 0))
    });
    let (k, _, w) = resolve_single(
      list,
      vec![
        (
          (10, 0),
          Object::Array(vec![
            Object::string_literal("a"),
            Object::string_literal("c"),
          ]),
        ),
        (
          (11, 0),
          Object::Array(vec![
            Object::string_literal("a"),
            Object::string_literal("b"),
            Object::string_literal("c"),
          ]),
        ),
      ],
    );
    match k {
      FieldKind::ListBox {
        options, values, ..
      } => {
        assert_eq!(values, Some(vec!["a".into(), "c".into()]));
        assert_eq!(options.len(), 3);
      }
      other => panic!("expected ListBox, got {}", kind_label(&other)),
    }
    assert!(w.is_empty());
  }

  #[test]
  fn non_string_values_and_actions() {
    let text = |v: Object| {
      widget_dict(
        dictionary! { "T" => Object::string_literal("amount"), "FT" => name("Tx"), "V" => v },
      )
    };
    let (_, c, _) = resolve_single(text(Object::Integer(42)), vec![]);
    assert_eq!(c.value, Some("42".into()));
    let (_, c, _) = resolve_single(text(Object::Dictionary(Dictionary::new())), vec![]);
    assert_eq!(c.value, None);
    let mut with_action = text(Object::string_literal("x"));
    with_action.set(
      "A",
      Object::Dictionary(dictionary! { "S" => name("ResetForm") }),
    );
    let (_, c, _) = resolve_single(with_action, vec![]);
    assert!(c.has_actions);
    let (_, c, _) = resolve_single(text(Object::string_literal("x")), vec![]);
    assert!(!c.has_actions);
  }

  // ── Task 4: emission, sorting, FormInfo, extract_forms ───────

  fn common() -> FieldCommon {
    FieldCommon {
      page: 1,
      rect: vec![],
      object_id: "0 0 obj".into(),
      field_object_id: "0 0 obj".into(),
      name: String::new(),
      partial_name: String::new(),
      widget_index: 0,
      widget_count: 1,
      value: None,
      default_value: None,
      tooltip: None,
      mapping_name: None,
      read_only: false,
      required: false,
      no_export: false,
      has_actions: false,
    }
  }

  /// Catalog (1,0) with optional /AcroForm, pages (2,0), page k at (100+k, 0).
  fn pdf_full(
    acroform: Option<Dictionary>,
    annots: Vec<Object>,
    extras: Vec<(ObjectId, Object)>,
  ) -> Document {
    let mut d = pdf_with_pages(annots, extras);
    if let Some(af) = acroform {
      let cat = d.get_dictionary_mut((1, 0)).unwrap();
      cat.set("AcroForm", Object::Dictionary(af));
    }
    d
  }

  fn acroform(fields: &[(u32, u16)]) -> Dictionary {
    dictionary! { "Fields" => refs(fields) }
  }

  fn merged_text(t: &str, v: &str, r: Object) -> Dictionary {
    widget_dict(
      dictionary! { "T" => Object::string_literal(t), "FT" => name("Tx"), "V" => Object::string_literal(v), "Rect" => r },
    )
  }

  fn rect_at(x0: i64, y0: i64, x1: i64, y1: i64) -> Object {
    Object::Array(vec![
      Object::Integer(x0),
      Object::Integer(y0),
      Object::Integer(x1),
      Object::Integer(y1),
    ])
  }

  #[test]
  fn no_acroform_no_widgets_gives_none_and_empty() {
    let d = pdf_full(None, vec![Object::Array(vec![])], vec![]);
    let out = extract_forms(&d);
    assert!(out.info.is_none());
    assert!(out.fields.is_empty());
  }

  #[test]
  fn widgets_without_acroform_become_unknown_with_info() {
    let d = pdf_full(
      None,
      vec![
        refs(&[(5, 0)]),
        Object::Array(vec![widget_dict(dictionary! { "T" => Object::string_literal("inline") }).into()]),
      ],
      vec![((5, 0), widget_dict(dictionary! { "T" => Object::string_literal("ghost"), "V" => Object::string_literal("boo") }).into())],
    );
    let out = extract_forms(&d);
    assert_eq!(out.fields.len(), 2);
    assert!(out.fields.iter().all(|f| f.kind_name() == "Unknown"));
    assert_eq!(
      out
        .fields
        .iter()
        .map(|f| f.name().to_string())
        .collect::<Vec<_>>(),
      vec!["ghost", "inline"]
    );
    assert_eq!(out.fields[1].object_id(), "page 2 annot 0");
    let info = out.info.expect("widgets imply info");
    assert!(!info.has_acro_form);
    assert_eq!(info.field_count, 0);
    assert_eq!(info.widget_count, 2);
  }

  #[test]
  fn unplaced_field_goes_to_info_not_array() {
    let d = pdf_full(
      Some(acroform(&[(10, 0)])),
      vec![],
      vec![((10, 0), dictionary! { "T" => Object::string_literal("hidden"), "FT" => name("Tx"), "V" => Object::string_literal("secret") }.into())],
    );
    let out = extract_forms(&d);
    assert!(out.fields.is_empty());
    let info = out.info.unwrap();
    assert_eq!(info.field_count, 1);
    assert_eq!(info.widget_count, 0);
    assert_eq!(info.unplaced_fields.len(), 1);
    let u = &info.unplaced_fields[0];
    assert_eq!(u.name, "hidden");
    assert_eq!(u.kind, "Text");
    assert_eq!(u.value, Some("secret".into()));
    assert_eq!(u.field_object_id, "10 0 obj");
  }

  #[test]
  fn unplaced_multiselect_keeps_values_options_and_flags() {
    let d = pdf_full(
      Some(acroform(&[(10, 0)])),
      vec![],
      vec![((10, 0), dictionary! {
        "T" => Object::string_literal("tags"),
        "FT" => name("Ch"),
        "Ff" => Object::Integer(1 << 21),
        "Opt" => Object::Array(vec![Object::string_literal("a"), Object::string_literal("b"), Object::string_literal("c")]),
        "V" => Object::Array(vec![Object::string_literal("a"), Object::string_literal("c")])
      }.into())],
    );
    let info = extract_forms(&d).info.unwrap();
    let u = &info.unplaced_fields[0];
    assert_eq!(u.kind, "ListBox");
    assert_eq!(u.values, Some(vec!["a".into(), "c".into()]));
    assert_eq!(u.options.as_ref().map(|o| o.len()), Some(3));
    assert!(u.multi_select);
    assert_eq!(u.value, Some("a".into()));
  }

  #[test]
  fn inline_widget_kid_leaves_parent_unplaced_under_its_own_id() {
    let d = pdf_full(
      Some(acroform(&[(5, 0)])),
      vec![Object::Array(vec![])],
      vec![(
        (5, 0),
        dictionary! {
          "T" => Object::string_literal("inlinekid"),
          "FT" => name("Tx"),
          "Kids" => Object::Array(vec![widget_dict(Dictionary::new()).into()])
        }
        .into(),
      )],
    );
    let out = extract_forms(&d);
    assert!(out.fields.is_empty());
    let info = out.info.unwrap();
    assert_eq!(info.unplaced_fields.len(), 1);
    assert_eq!(info.unplaced_fields[0].field_object_id, "5 0 obj");
    assert_eq!(info.unplaced_fields[0].name, "inlinekid");
  }

  #[test]
  fn inline_named_field_is_unplaced_with_synthetic_id() {
    let d = pdf_full(
      Some(acroform(&[(5, 0)])),
      vec![],
      vec![((5, 0), dictionary! {
        "T" => Object::string_literal("p"),
        "Kids" => Object::Array(vec![dictionary! { "T" => Object::string_literal("sub"), "FT" => name("Tx"), "V" => Object::string_literal("x") }.into()])
      }.into())],
    );
    let info = extract_forms(&d).info.unwrap();
    assert_eq!(info.field_count, 1);
    assert_eq!(info.unplaced_fields.len(), 1);
    assert_eq!(info.unplaced_fields[0].field_object_id, "5 0 obj/kid 0");
    assert_eq!(info.unplaced_fields[0].name, "p.sub");
    assert_eq!(info.unplaced_fields[0].value, Some("x".into()));
  }

  #[test]
  fn unplaced_signature_reports_signed() {
    let sig = |v: Option<Object>| {
      let mut f = dictionary! { "T" => Object::string_literal("sig"), "FT" => name("Sig") };
      if let Some(v) = v {
        f.set("V", v);
      }
      pdf_full(
        Some(acroform(&[(10, 0)])),
        vec![],
        vec![((10, 0), f.into())],
      )
    };
    let info = extract_forms(&sig(Some(Object::Dictionary(
      dictionary! { "Type" => name("Sig") },
    ))))
    .info
    .unwrap();
    assert_eq!(info.unplaced_fields[0].kind, "Signature");
    assert!(info.unplaced_fields[0].signed);
    let info = extract_forms(&sig(None)).info.unwrap();
    assert!(!info.unplaced_fields[0].signed);
  }

  #[test]
  fn truncation_and_dangling_refs_surface_in_info() {
    let mut objects = vec![];
    for i in 10..160u32 {
      let mut dict = dictionary! { "T" => Object::string_literal(format!("n{i}")) };
      if i < 159 {
        dict.set("Kids", refs(&[(i + 1, 0)]));
      } else {
        dict.set("FT", name("Tx"));
      }
      objects.push(((i, 0), dict.into()));
    }
    let d = pdf_full(Some(acroform(&[(10, 0), (999, 0)])), vec![], objects);
    let info = extract_forms(&d).info.unwrap();
    assert!(info.truncated);
    assert!(info.warnings.len() >= 2, "{:?}", info.warnings);
  }

  #[test]
  fn multi_page_field_emits_one_entry_per_placement() {
    let kid = |page_rect: Object| {
      let mut k = widget_dict(dictionary! { "Rect" => page_rect });
      k.set("Parent", Object::Reference((10, 0)));
      k
    };
    let d = pdf_full(
      Some(acroform(&[(10, 0)])),
      vec![Object::Array(vec![]), refs(&[(11, 0)]), refs(&[(12, 0)])],
      vec![
        ((10, 0), dictionary! { "T" => Object::string_literal("id"), "FT" => name("Tx"), "V" => Object::string_literal("A-1"), "Kids" => refs(&[(11, 0), (12, 0)]) }.into()),
        ((11, 0), kid(rect_at(50, 700, 200, 720)).into()),
        ((12, 0), kid(rect_at(50, 700, 200, 720)).into()),
      ],
    );
    let out = extract_forms(&d);
    assert_eq!(out.fields.len(), 2);
    let idx: Vec<u32> = out
      .fields
      .iter()
      .map(|f| match f {
        PageFormField::Text { widget_index, .. } => *widget_index,
        _ => panic!(),
      })
      .collect();
    assert_eq!(idx, vec![0, 1]);
    assert!(out.fields.iter().all(|f| match f {
      PageFormField::Text {
        widget_count,
        field_object_id,
        ..
      } => *widget_count == 2 && field_object_id == "10 0 obj",
      _ => false,
    }));
    assert_eq!(form_field_pages(&out.fields), vec![2, 3]);
    let info = out.info.unwrap();
    assert_eq!(info.field_count, 1);
    assert_eq!(info.widget_count, 2);
  }

  #[test]
  fn duplicate_names_keep_distinct_field_object_ids() {
    let d = pdf_full(
      Some(acroform(&[(10, 0), (11, 0)])),
      vec![refs(&[(10, 0), (11, 0)])],
      vec![
        (
          (10, 0),
          merged_text("dup", "one", rect_at(50, 600, 200, 620)).into(),
        ),
        (
          (11, 0),
          merged_text("dup", "two", rect_at(50, 560, 200, 580)).into(),
        ),
      ],
    );
    let out = extract_forms(&d);
    assert_eq!(out.fields.len(), 2);
    let ids: Vec<&str> = out
      .fields
      .iter()
      .map(|f| match f {
        PageFormField::Text {
          field_object_id, ..
        } => field_object_id.as_str(),
        _ => panic!(),
      })
      .collect();
    assert_ne!(ids[0], ids[1]);
    assert!(out.fields.iter().all(|f| f.name() == "dup"));
  }

  #[test]
  fn sort_is_page_then_top_down_then_left_right_then_name() {
    let entry = |name: &str, page: u32, rect: Vec<f64>| {
      let mut c = common();
      c.name = name.into();
      c.page = page;
      c.rect = rect;
      PageFormField::new(c, FieldKind::Unknown {})
    };
    let mut fields = vec![
      entry("z", 2, vec![0.0, 0.0, 10.0, 10.0]),
      entry("b", 1, vec![50.0, 700.0, 100.0, 720.0]),
      entry("c", 1, vec![50.0, 600.0, 100.0, 620.0]),
      entry("d", 1, vec![10.0, 700.0, 20.0, 720.0]),
      entry("e", 1, vec![0.0, 0.0, 100.0]),
      entry("a", 1, vec![50.0, 700.0, 100.0, 720.0]),
    ];
    sort_fields(&mut fields);
    assert_eq!(
      fields
        .iter()
        .map(|f| f.name().to_string())
        .collect::<Vec<_>>(),
      vec!["d", "a", "b", "c", "e", "z"]
    );
  }

  #[test]
  fn field_count_excludes_unknown_and_counts_unplaced() {
    let d = pdf_full(
      Some(acroform(&[(10, 0), (11, 0)])),
      vec![refs(&[(10, 0), (12, 0)])],
      vec![
        (
          (10, 0),
          merged_text("placed", "p", rect_at(0, 0, 10, 10)).into(),
        ),
        (
          (11, 0),
          dictionary! { "T" => Object::string_literal("hidden"), "FT" => name("Tx") }.into(),
        ),
        (
          (12, 0),
          widget_dict(dictionary! { "T" => Object::string_literal("orphan") }).into(),
        ),
      ],
    );
    let out = extract_forms(&d);
    let info = out.info.unwrap();
    assert_eq!(info.field_count, 2);
    assert_eq!(info.widget_count, 2);
    assert_eq!(out.fields.len(), 2);
    assert_eq!(info.unplaced_fields.len(), 1);
  }

  #[test]
  fn acroform_dictionary_flags_are_reported() {
    let mut af = acroform(&[]);
    af.set("NeedAppearances", Object::Boolean(true));
    af.set("SigFlags", Object::Integer(3));
    af.set("XFA", Object::Array(vec![]));
    let info = extract_forms(&pdf_full(Some(af), vec![], vec![]))
      .info
      .unwrap();
    assert!(info.need_appearances);
    assert_eq!(info.sig_flags, 3);
    assert!(info.has_xfa);
    let info = extract_forms(&pdf_full(Some(acroform(&[])), vec![], vec![]))
      .info
      .unwrap();
    assert!(!info.need_appearances);
    assert_eq!(info.sig_flags, 0);
    assert!(!info.has_xfa);
    assert!(info.has_acro_form);
  }
}
