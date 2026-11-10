use super::*;

fn common() -> FieldCommon {
  FieldCommon {
    page: 2,
    rect: vec![1.0, 2.0, 3.0, 4.0],
    object_id: "7 0 obj".into(),
    field_object_id: "5 0 obj".into(),
    name: "a.b".into(),
    partial_name: "b".into(),
    widget_index: 1,
    widget_count: 2,
    value: Some("v".into()),
    default_value: None,
    tooltip: None,
    mapping_name: None,
    read_only: false,
    required: false,
    no_export: false,
    has_actions: false,
  }
}

#[test]
fn new_copies_common_fields_into_every_variant() {
  let f = PageFormField::new(
    common(),
    FieldKind::Checkbox {
      on_state: Some("Ja".into()),
      export_value: Some("Ja".into()),
      checked: true,
    },
  );
  assert_eq!(f.page(), 2);
  assert_eq!(f.rect(), &[1.0, 2.0, 3.0, 4.0]);
  assert_eq!(f.name(), "a.b");
  assert_eq!(f.object_id(), "7 0 obj");
  assert_eq!(f.kind_name(), "Checkbox");
  match f {
    PageFormField::Checkbox {
      on_state,
      export_value,
      checked,
      widget_count,
      ..
    } => {
      assert_eq!(on_state.as_deref(), Some("Ja"));
      assert_eq!(export_value.as_deref(), Some("Ja"));
      assert!(checked);
      assert_eq!(widget_count, 2);
    }
    _ => panic!("expected Checkbox"),
  }
}

#[test]
fn unit_like_variants_still_carry_common_fields() {
  let f = PageFormField::new(common(), FieldKind::Unknown {});
  assert_eq!(f.kind_name(), "Unknown");
  assert_eq!(f.page(), 2);
}
