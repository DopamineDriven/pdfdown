//! Read-only AcroForm field extraction (spec: notes/2026-11-01.md §5).
//!
//! This module walks `/AcroForm /Fields` into an arena of [`FieldNode`]s, resolves the
//! inheritable attributes once per logical field, and (in later tasks) joins fields to the
//! widget annotations placed on pages. Nothing here writes to the document.

use crate::types::ChoiceOption;
use lopdf::{Dictionary, Document, Object, ObjectId};
use std::collections::HashSet;

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
  pub(crate) fn no_toggle_to_off(self) -> bool {
    self.bit(15)
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
  pub rect: Option<Vec<f64>>,
  pub appearance_state: Option<String>,
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
    let rect = get(b"Rect")
      .and_then(|o| deref(doc, o, warnings, "/Rect"))
      .map(parse_rect);
    let appearance_state = get(b"AS")
      .and_then(|o| deref(doc, o, warnings, "/AS"))
      .and_then(|o| match o {
        Object::Name(n) => Some(decode_name_bytes(n)),
        _ => None,
      });

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
      rect,
      appearance_state,
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
    node.partial_name.is_empty() && node.parent.is_some()
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
}
