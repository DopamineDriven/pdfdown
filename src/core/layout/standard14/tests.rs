use super::*;
#[test]
fn tables_cover_the_printable_ascii_range() {
  let h = lookup("Helvetica", false);
  assert_eq!(h.width(32), Some(0.278));
  assert_eq!(h.width(b'H' as u32), Some(0.722));
  assert_eq!(h.width(b'~' as u32), Some(0.584));
  assert_eq!(h.width(31), None);
  assert_eq!(lookup("Arial,Bold", true).width(b'A' as u32), Some(0.722));
  assert_eq!(lookup("Times-Roman", false).width(b'A' as u32), Some(0.722));
  assert_eq!(
    lookup("TimesNewRoman,Bold", true).width(b'W' as u32),
    Some(1.0)
  );
  assert_eq!(lookup("Courier", false).width(b'i' as u32), Some(0.6));
}
