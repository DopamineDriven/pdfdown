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
        dictionary! { "Type" => name("Annot"), "Subtype" => name("Link"), "Rect" => rect() }.into(),
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
fn radio_without_as_selects_by_group_value_through_ap_states() {
  // No /AS anywhere: the group /V "1" selects the widget whose /AP names state "1".
  let kid = |state: &str| dictionary! { "AP" => ap_n(&["Off", state]) };
  let (out, w) = radio_group(
    dictionary! { "V" => name("1") },
    vec![kid("0"), kid("1"), kid("2")],
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
    parts.iter().map(|p| p.2).collect::<Vec<_>>(),
    vec![false, true, false]
  );
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

// ── Final review fixes ────────────────────────────────────────

#[test]
fn self_referencing_array_value_does_not_recurse() {
  let d = doc(vec![
    ((5, 0), Object::Array(vec![Object::Reference((5, 0))])),
    ((6, 0), Object::Array(vec![Object::Reference((7, 0))])),
    ((7, 0), Object::string_literal("x")),
    (
      (8, 0),
      Object::Array(vec![Object::Array(vec![Object::string_literal("deep")])]),
    ),
  ]);
  let mut w = vec![];
  assert_eq!(
    decode_text_object(&Object::Reference((5, 0)), &d, &mut w),
    None
  );
  assert_eq!(
    decode_text_object(&Object::Reference((6, 0)), &d, &mut w),
    Some("x".into())
  );
  assert_eq!(
    decode_text_object(&Object::Reference((8, 0)), &d, &mut w),
    None
  );
}

#[test]
fn scan_dedupes_repeated_widget_references() {
  let d = pdf_with_pages(
    vec![refs(&[(5, 0), (5, 0)]), refs(&[(5, 0)])],
    vec![(
      (5, 0),
      widget_dict(dictionary! { "T" => Object::string_literal("w") }).into(),
    )],
  );
  let mut w = vec![];
  let widgets = scan_widgets(&d, &mut w);
  assert_eq!(widgets.len(), 1);
  assert_eq!(widgets[0].page, 1);
  assert_eq!(w.iter().filter(|m| m.contains("repeats widget")).count(), 2);
}

#[test]
fn orphan_widget_inherits_name_value_and_flags_through_parent() {
  let mut kid = widget_dict(Dictionary::new());
  kid.set("Parent", Object::Reference((5, 0)));
  let d = pdf_full(
      Some(acroform(&[])),
      vec![refs(&[(6, 0)])],
      vec![
        ((4, 0), dictionary! { "T" => Object::string_literal("top"), "Kids" => refs(&[(5, 0)]) }.into()),
        ((5, 0), dictionary! {
          "T" => Object::string_literal("orphanParent"), "FT" => name("Tx"), "V" => Object::string_literal("val"),
          "Ff" => Object::Integer(1), "Parent" => Object::Reference((4, 0)), "Kids" => refs(&[(6, 0)])
        }.into()),
        ((6, 0), kid.into()),
      ],
    );
  let out = extract_forms(&d);
  assert_eq!(out.fields.len(), 1);
  assert_eq!(out.fields[0].kind_name(), "Unknown");
  assert_eq!(out.fields[0].name(), "top.orphanParent");
  match &out.fields[0] {
    PageFormField::Unknown {
      value,
      read_only,
      partial_name,
      ..
    } => {
      assert_eq!(value.as_deref(), Some("val"));
      assert!(*read_only);
      assert_eq!(partial_name, "");
    }
    _ => unreachable!(),
  }
  let info = out.info.unwrap();
  assert_eq!(info.field_count, 0);
  assert_eq!(info.widget_count, 1);
}

#[test]
fn dangling_acroform_reference_reports_info_with_warning() {
  let mut d = pdf_with_pages(vec![Object::Array(vec![])], vec![]);
  d.get_dictionary_mut((1, 0))
    .unwrap()
    .set("AcroForm", Object::Reference((999, 0)));
  let out = extract_forms(&d);
  let info = out
    .info
    .expect("an /AcroForm key must report info even when it cannot be resolved");
  assert!(!info.has_acro_form);
  assert_eq!(info.field_count, 0);
  assert!(
    info
      .warnings
      .iter()
      .any(|w| w.contains("/AcroForm") && w.contains("999 0 R")),
    "{:?}",
    info.warnings
  );
}
