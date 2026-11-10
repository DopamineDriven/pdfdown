use super::super::lexicon::Field;
use super::super::segments::label_hits;
use super::super::testing::{page, segments_at};
use super::*;

#[test]
fn a_totals_label_takes_every_amount_to_its_right_and_prefers_the_last_column() {
  let segs = segments_at(&page(&[
    (180.0, 306.0, 10.0, false, "Time & Materials"),
    (306.0, 306.0, 10.0, false, "Flat Rate"),
    (418.0, 306.0, 10.0, false, "Total"),
    (81.0, 297.0, 10.0, false, "Labor:"),
    (197.0, 297.0, 10.0, false, "$1,200.00"),
    (323.0, 297.0, 10.0, false, "$300.00"),
    (413.0, 297.0, 10.0, false, "$1,500.00"),
    (81.0, 249.0, 10.0, false, "Sub Total:"),
    (191.0, 249.0, 10.0, false, "$1,200.00"),
    (299.0, 249.0, 10.0, false, "$300.00"),
    (407.0, 249.0, 10.0, false, "$1,500.00"),
    (81.0, 172.0, 10.0, false, "Total Due:"),
    (191.0, 172.0, 10.0, false, "$1,500.00"),
    (299.0, 172.0, 10.0, false, "$0.00"),
    (330.0, 700.0, 10.0, false, "Invoice Date:"),
    (420.0, 700.0, 10.0, false, "11/06/2026"),
    (500.0, 700.0, 10.0, false, "Original"),
  ]));
  let hits = label_hits(&segs);
  let subs: Vec<(String, f64)> = candidates(&segs, &hits, Field::SubTotal)
    .into_iter()
    .map(|c| (c.text, c.geometry_score))
    .collect();
  assert!(
    subs.contains(&("$1,500.00".to_string(), 0.8)),
    "under the Total header: {subs:?}"
  );
  assert!(subs.contains(&("$1,200.00".to_string(), 0.6)), "{subs:?}");
  assert!(subs.contains(&("$300.00".to_string(), 0.6)), "{subs:?}");
  let dues: Vec<(String, f64)> = candidates(&segs, &hits, Field::AmountDue)
    .into_iter()
    .map(|c| (c.text, c.geometry_score))
    .collect();
  assert!(
    dues.contains(&("$0.00".to_string(), 0.7)) && dues.contains(&("$1,500.00".to_string(), 0.6)),
    "no Total header over these columns: the last column leads: {dues:?}"
  );
  let dates = candidates(&segs, &hits, Field::InvoiceDate);
  assert!(
    !dates.is_empty() && dates.iter().all(|c| c.text == "11/06/2026"),
    "a date takes only the nearest segment to its right: {:?}",
    dates.iter().map(|c| &c.text).collect::<Vec<_>>()
  );
}

#[test]
fn same_line_right_then_below_then_next_column() {
  let segs = segments_at(&page(&[(
    348.0,
    723.0,
    10.0,
    false,
    "Invoice Number INV-3337",
  )]));
  let c = candidates(&segs, &label_hits(&segs), Field::InvoiceId);
  assert_eq!(c[0].text, "INV-3337");
  assert_eq!(c[0].geometry_score, 1.0);
  assert_eq!(c[0].label_score, 1.0);
  assert_eq!(c[0].page_index, 0);
  assert_eq!(c[0].words.len(), 1);

  let segs = segments_at(&page(&[
    (50.0, 690.0, 10.0, false, "Bill To:"),
    (50.0, 676.0, 10.0, false, "Skyline Charter LLC"),
  ]));
  let c = candidates(&segs, &label_hits(&segs), Field::BillTo);
  assert_eq!(c[0].text, "Skyline Charter LLC");
  assert_eq!(c[0].geometry_score, 0.8);

  let segs = segments_at(&page(&[(50.0, 690.0, 10.0, false, "Terms: Net 30")]));
  assert!(candidates(&segs, &label_hits(&segs), Field::InvoiceDate).is_empty());

  assert_eq!(score(1.0, 1.0, 1.0, None), 1.0);
  assert!((score(1.0, 1.0, 0.8, Some(0.5)) - 0.48).abs() < 1e-9);
}
