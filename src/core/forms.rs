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
    // A text value is at most one array deep (multi-select /V). Never recurse: a
    // self-referencing array (`5 0 obj [5 0 R]`) would otherwise overflow the stack.
    Object::Array(items) => scalar_to_string(deref(doc, items.first()?, warnings, "value")?),
    other => scalar_to_string(other),
  }
}

/// Text string, name or number → `String`; anything else (dictionary, stream, array) → `None`.
fn scalar_to_string(obj: &Object) -> Option<String> {
  match obj {
    Object::String(bytes, _) => Some(
      lopdf::decode_text_string(obj)
        .unwrap_or_else(|_| String::from_utf8_lossy(bytes).into_owned()),
    ),
    Object::Name(n) => Some(decode_name_bytes(n)),
    Object::Integer(i) => Some(i.to_string()),
    Object::Real(r) => Some(r.to_string()),
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
  // A widget annotation has one page; a reference repeated in /Annots (or on two pages,
  // which incremental saves and merges produce) is reported once and warned about.
  let mut seen: HashSet<ObjectId> = HashSet::new();
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
      if let Some(id) = key
        && !seen.insert(id)
      {
        warnings.push(format!(
          "page {page_num}: /Annots[{index}] repeats widget {} {} R",
          id.0, id.1
        ));
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

  // 2. /AcroForm, if present. The key's presence is tracked separately from its resolution so
  //    a dangling reference reports `FormInfo { has_acro_form: false, warnings }` instead of
  //    silence: "could not extract" must stay distinguishable from "nothing present".
  let has_acroform_key = doc.catalog().ok().is_some_and(|cat| cat.has(b"AcroForm"));
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
  let info = (has_acroform_key || !fields.is_empty()).then(|| FormInfo {
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

/// A widget annotation no field claims, typed `Unknown`. Its `/Parent` chain is still walked
/// (visited set, depth cap) so a widget whose field never made it into `/AcroForm /Fields`, a
/// common breakage, keeps its dotted name and inherits `/V`, `/DV` and `/Ff` like a placed one.
fn unknown_for(doc: &Document, widget: &PlacedWidget, warnings: &mut Vec<String>) -> PageFormField {
  // chain[0] is the widget itself, then each /Parent up to the root.
  let mut chain: Vec<&Dictionary> = vec![&widget.dict];
  let mut visited: HashSet<ObjectId> = widget.key.into_iter().collect();
  while chain.len() < MAX_FIELD_DEPTH {
    let Ok(Object::Reference(pid)) = chain[chain.len() - 1].get(b"Parent") else {
      break;
    };
    if !visited.insert(*pid) {
      warnings.push(format!(
        "page {}: /Parent chain of widget {} loops at {} {} R",
        widget.page, widget.object_id, pid.0, pid.1
      ));
      break;
    }
    match doc.get_dictionary(*pid) {
      Ok(parent) => chain.push(parent),
      Err(_) => {
        warnings.push(format!(
          "page {}: /Parent unresolved reference {} {} R",
          widget.page, pid.0, pid.1
        ));
        break;
      }
    }
  }
  let own = |key: &[u8], warnings: &mut Vec<String>| {
    widget
      .dict
      .get(key)
      .ok()
      .and_then(|o| decode_text_object(o, doc, warnings))
  };
  let inherited = |key: &[u8]| chain.iter().find_map(|d| d.get(key).ok());
  let mut names: Vec<String> = chain
    .iter()
    .filter_map(|d| d.get(b"T").ok())
    .filter_map(|o| decode_text_object(o, doc, warnings))
    .filter(|n| !n.is_empty())
    .collect();
  names.reverse();
  let flags = FieldFlags(
    inherited(b"Ff")
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
    name: names.join("."),
    partial_name: own(b"T", warnings).unwrap_or_default(),
    widget_index: 0,
    widget_count: 1,
    value: inherited(b"V").and_then(|o| decode_text_object(o, doc, warnings)),
    default_value: inherited(b"DV").and_then(|o| decode_text_object(o, doc, warnings)),
    tooltip: own(b"TU", warnings),
    mapping_name: own(b"TM", warnings),
    read_only: flags.read_only(),
    required: flags.required(),
    no_export: flags.no_export(),
    has_actions: widget.dict.has(b"AA") || widget.dict.has(b"A"),
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
mod tests;
