use super::super::amounts::Amount;
use super::super::lexicon::Field;
use super::*;
use crate::types::FieldSpan;

fn cand(field: Field, value: f64, y: f64) -> TotalCandidate {
  TotalCandidate {
    field,
    amount: Amount {
      value: value.abs(),
      negative: value < 0.0,
      code: None,
      symbol: Some("$".into()),
    },
    text: format!("${value:.2}"),
    span: FieldSpan {
      page: 1,
      x0: 400.0,
      y0: y,
      x1: 500.0,
      y1: y + 10.0,
    },
    base: 0.76,
  }
}

#[test]
fn subtotal_plus_tax_reconciles_and_lifts_every_participant() {
  let t = resolve(
    &[
      cand(Field::SubTotal, 85.0, 400.0),
      cand(Field::Tax, 8.5, 388.0),
      cand(Field::Total, 93.5, 376.0),
      cand(Field::AmountDue, 93.5, 300.0),
    ],
    None,
  );
  assert!(t.reconciled);
  assert_eq!(t.invoice_total.as_ref().unwrap().value, 93.5);
  assert_eq!(t.amount_due.as_ref().unwrap().value, 93.5);
  assert_eq!(t.sub_total.as_ref().unwrap().value, 85.0);
  assert_eq!(t.total_tax.as_ref().unwrap().value, 8.5);
  for f in [&t.sub_total, &t.total_tax, &t.invoice_total, &t.amount_due] {
    assert!(
      (f.as_ref().unwrap().confidence - 0.96).abs() < 1e-9,
      "{}",
      f.as_ref().unwrap().confidence
    );
  }
  assert_eq!(
    t.invoice_total.as_ref().unwrap().currency_symbol.as_deref(),
    Some("$")
  );
  assert!(!t.negative);
}

#[test]
fn several_tax_lines_sum_and_a_due_amount_stands_in_for_a_missing_total() {
  let t = resolve(
    &[
      cand(Field::SubTotal, 4500.0, 300.0),
      cand(Field::Tax, 225.0, 286.0),
      cand(Field::Tax, 448.88, 272.0),
      cand(Field::Total, 5173.88, 256.0),
    ],
    Some("CAD"),
  );
  assert!(t.reconciled);
  assert!((t.total_tax.as_ref().unwrap().value - 673.88).abs() < 1e-9);
  assert_eq!(
    t.invoice_total.as_ref().unwrap().currency_code.as_deref(),
    Some("CAD")
  );

  let t = resolve(
    &[
      cand(Field::SubTotal, 12450.0, 300.0),
      cand(Field::Tax, 871.5, 286.0),
      cand(Field::AmountDue, 13321.5, 270.0),
    ],
    None,
  );
  assert!(t.reconciled);
  assert_eq!(t.invoice_total.as_ref().unwrap().value, 13321.5);
  assert_eq!(
    t.invoice_total.as_ref().unwrap().span.as_ref().unwrap().y0,
    270.0,
    "the due amount's span"
  );
  assert_eq!(t.amount_due.as_ref().unwrap().value, 13321.5);
}

#[test]
fn exact_reconciliation_beats_one_within_a_percent_and_a_due_line_alone_is_the_total() {
  let t = resolve(
    &[
      cand(Field::SubTotal, 85.0, 400.0),
      cand(Field::Tax, 8.0, 500.0),
      cand(Field::Tax, 8.5, 388.0),
      cand(Field::Total, 93.5, 376.0),
    ],
    None,
  );
  assert_eq!(
    t.total_tax.as_ref().unwrap().value,
    8.5,
    "the exact tax, not the column item within 1%"
  );

  let mut page1 = cand(Field::Total, 10.0, 100.0);
  page1.span.page = 1;
  let mut page2 = cand(Field::Total, 250.0, 400.0);
  page2.span.page = 2;
  let t = resolve(&[page1, page2], None);
  assert_eq!(
    t.invoice_total.as_ref().unwrap().value,
    250.0,
    "the last page's total, not page 1's"
  );

  let t = resolve(
    &[
      cand(Field::Total, 93.5, 300.0),
      cand(Field::AmountDue, -12.5, 280.0),
    ],
    None,
  );
  assert!(!t.negative, "an overpaid balance is not a credit note");

  let t = resolve(&[cand(Field::AmountDue, 93.5, 300.0)], None);
  assert_eq!(
    t.invoice_total.as_ref().unwrap().value,
    93.5,
    "an amount due alone is the total"
  );
  assert_eq!(t.amount_due.as_ref().unwrap().value, 93.5);
}

#[test]
fn a_total_beside_a_totals_block_outranks_a_lone_total_on_a_later_page() {
  let sub = cand(Field::SubTotal, 900.0, 300.0);
  let total = cand(Field::Total, 1000.0, 250.0);
  let mut lone = cand(Field::Total, 412.8, 500.0);
  lone.span.page = 2;
  let t = resolve(&[sub, total, lone], None);
  assert!(!t.reconciled, "900 is not 1000");
  assert_eq!(
    t.invoice_total.as_ref().unwrap().value,
    1000.0,
    "the page with the totals block, not the later page"
  );
}

#[test]
fn without_a_reconciliation_the_lowest_total_wins_and_a_negative_total_is_flagged() {
  let t = resolve(
    &[
      cand(Field::Total, 10.0, 500.0),
      cand(Field::Total, 250.0, 300.0),
    ],
    None,
  );
  assert!(!t.reconciled);
  assert_eq!(t.invoice_total.as_ref().unwrap().value, 250.0);
  assert!(
    (t.invoice_total.as_ref().unwrap().confidence - 0.76).abs() < 1e-9,
    "no bonus"
  );

  let t = resolve(&[cand(Field::Total, -1200.0, 300.0)], None);
  assert!(t.negative);
  assert_eq!(
    t.invoice_total.as_ref().unwrap().value,
    1200.0,
    "credit amounts are absolute"
  );
}
