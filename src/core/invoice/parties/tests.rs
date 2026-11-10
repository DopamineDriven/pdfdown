use super::super::lexicon::Field;
use super::super::segments::label_hits;
use super::super::testing::{acme_page, page, segments_at};
use super::*;

#[test]
fn the_vendor_is_the_largest_non_title_line_in_the_top_band_with_its_address() {
  let p = acme_page();
  let segs = segments_at(&p);
  let v = vendor(&segs, &label_hits(&segs), p.height).unwrap();
  let name = v.name.unwrap();
  assert_eq!(name.value, "ACME AVIATION SERVICES");
  assert_eq!(name.span.unwrap().page, 1);
  assert_eq!(
    v.address.unwrap().lines,
    ["1200 Hangar Road", "Wichita, KS 67209"]
  );

  let bare = page(&[
    (450.0, 760.0, 16.0, true, "INVOICE"),
    (380.0, 730.0, 10.0, false, "Invoice No: 1"),
  ]);
  let bare_segs = segments_at(&bare);
  assert!(vendor(&bare_segs, &label_hits(&bare_segs), bare.height).is_none());
}

#[test]
fn a_label_alone_in_its_column_names_the_party_in_the_next_column() {
  let p = page(&[
    (380.0, 730.0, 10.0, false, "CLIENTE:"),
    (520.0, 730.0, 10.0, false, "Skyline Charter LLC"),
    (520.0, 716.0, 10.0, false, "88 Terminal Drive"),
    (380.0, 716.0, 10.0, false, "FECHA:"),
    (520.0, 702.0, 10.0, false, "Denver, CO 80249"),
  ]);
  let segs = segments_at(&p);
  let c = party_block(&segs, &label_hits(&segs), Field::BillTo).unwrap();
  assert_eq!(c.name.unwrap().value, "Skyline Charter LLC");
  assert_eq!(
    c.address.unwrap().lines,
    vec!["88 Terminal Drive", "Denver, CO 80249"]
  );
}

#[test]
fn a_label_remainder_that_is_no_name_is_not_a_name() {
  let p = page(&[
    (50.0, 760.0, 14.0, true, "ACME AVIATION SERVICES"),
    (380.0, 730.0, 10.0, false, "Vendor #: 4471"),
    (380.0, 716.0, 10.0, false, "Customer No: 10045"),
    (50.0, 690.0, 10.0, false, "From 01/01/2024 to 01/31/2024"),
  ]);
  let segs = segments_at(&p);
  let hits = label_hits(&segs);
  assert!(
    party_block(&segs, &hits, Field::Vendor).is_none(),
    "`4471` and a service period are no vendor"
  );
  let customer = party_block(&segs, &hits, Field::BillTo).unwrap();
  assert!(customer.name.is_none());
  assert_eq!(customer.id.as_ref().unwrap().value, "10045");
}

#[test]
fn a_party_block_is_the_aligned_lines_under_its_label() {
  let p = acme_page();
  let segs = segments_at(&p);
  let c = party_block(&segs, &label_hits(&segs), Field::BillTo).unwrap();
  assert_eq!(c.name.unwrap().value, "Skyline Charter LLC");
  assert_eq!(
    c.address.unwrap().lines,
    ["88 Terminal Drive", "Denver, CO 80249"]
  );
  assert!(c.email.is_none() && c.phone.is_none());
}
