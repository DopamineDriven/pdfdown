use super::*;

#[test]
fn identifiers_keep_their_printed_form_without_label_remnants() {
  assert_eq!(parse_identifier("#13594027").as_deref(), Some("13594027"));
  assert_eq!(parse_identifier("INV-3337").as_deref(), Some("INV-3337"));
  assert_eq!(parse_identifier("A-10045,").as_deref(), Some("A-10045"));
  assert_eq!(
    parse_identifier("E240317201-CN").as_deref(),
    Some("E240317201-CN")
  );
  assert_eq!(
    parse_identifier("No. 2015-0831").as_deref(),
    Some("2015-0831")
  );
  assert_eq!(parse_identifier(": F-2209").as_deref(), Some("F-2209"));
  assert_eq!(parse_identifier("Invoice"), None, "no digit");
  assert_eq!(parse_identifier("12"), None, "too short");
  assert_eq!(
    parse_identifier("January 25, 2016"),
    None,
    "a date is not an identifier"
  );
}

#[test]
fn a_date_shape_is_not_an_identifier() {
  assert!(
    parse_identifier("08/07/7014").is_none(),
    "a misread date is still date-shaped"
  );
  assert!(parse_identifier("31.02.2019").is_none());
  assert!(parse_identifier("2014-13-40").is_none());
  assert_eq!(
    parse_identifier("2015-0831").as_deref(),
    Some("2015-0831"),
    "two groups are an id"
  );
  assert_eq!(
    parse_identifier("12-345-6789").as_deref(),
    Some("12-345-6789"),
    "a three-digit middle group is not a date"
  );
  assert_eq!(parse_identifier("03/14/2024"), None);
  assert_eq!(parse_identifier("31.08.2015"), None);
  assert_eq!(parse_identifier("2022-09-15"), None);
  assert_eq!(
    parse_identifier("2015-0831").as_deref(),
    Some("2015-0831"),
    "two parts are an id"
  );
}

#[test]
fn registrations_and_serials() {
  assert_eq!(parse_registration("N417SK").as_deref(), Some("N417SK"));
  assert_eq!(parse_registration("c-gxyz").as_deref(), Some("C-GXYZ"));
  assert_eq!(parse_registration("G-ABCD").as_deref(), Some("G-ABCD"));
  assert_eq!(parse_registration("12345"), None);
  assert_eq!(parse_registration("Nordique"), None);
  assert_eq!(parse_serial("20311").as_deref(), Some("20311"));
  assert_eq!(parse_serial("MSN-5521").as_deref(), Some("MSN-5521"));
  assert_eq!(parse_serial("S/N"), None);
  assert_eq!(parse_serial("A"), None);
}
