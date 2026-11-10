use super::*;

#[test]
fn parse_page_box_valid_integers() {
  let obj = Object::Array(vec![
    Object::Integer(0),
    Object::Integer(0),
    Object::Integer(612),
    Object::Integer(792),
  ]);
  let result = parse_page_box(&obj);
  assert_eq!(result, Some([0.0, 0.0, 612.0, 792.0]));
}

#[test]
fn parse_page_box_valid_reals() {
  let obj = Object::Array(vec![
    Object::Real(0.0),
    Object::Real(0.0),
    Object::Real(595.0),
    Object::Real(842.0),
  ]);
  let result = parse_page_box(&obj);
  assert_eq!(result, Some([0.0, 0.0, 595.0, 842.0]));
}

#[test]
fn parse_page_box_mixed_types() {
  let obj = Object::Array(vec![
    Object::Integer(0),
    Object::Real(0.5),
    Object::Integer(612),
    Object::Real(792.0),
  ]);
  let result = parse_page_box(&obj);
  assert_eq!(result, Some([0.0, 0.5, 612.0, 792.0]));
}

#[test]
fn parse_page_box_too_short() {
  let obj = Object::Array(vec![Object::Integer(0), Object::Integer(0)]);
  assert_eq!(parse_page_box(&obj), None);
}

#[test]
fn parse_page_box_non_numeric() {
  let obj = Object::Array(vec![
    Object::Integer(0),
    Object::Integer(0),
    Object::Name(b"bad".to_vec()),
    Object::Integer(792),
  ]);
  assert_eq!(parse_page_box(&obj), None);
}

#[test]
fn parse_page_box_not_array() {
  let obj = Object::Integer(42);
  assert_eq!(parse_page_box(&obj), None);
}

#[test]
fn parse_page_box_extra_elements_ignored() {
  let obj = Object::Array(vec![
    Object::Integer(0),
    Object::Integer(0),
    Object::Integer(612),
    Object::Integer(792),
    Object::Integer(999),
  ]);
  // Only first 4 used
  assert_eq!(parse_page_box(&obj), Some([0.0, 0.0, 612.0, 792.0]));
}
