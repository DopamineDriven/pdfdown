//! Totals: the subtotal, tax, total and amount-due candidates reconciled into one story. The
//! combination where `subTotal + tax ≈ invoiceTotal` wins and lifts every participant; without
//! one, the lowest `Total` on the page is the invoice total. Amounts are reported absolute; a
//! negative total is flagged for the credit decision.

use super::amounts::Amount;
use super::lexicon::Field;
use crate::types::{FieldSpan, MoneyField};

#[derive(Debug, Clone)]
pub(crate) struct TotalCandidate {
  pub field: Field,
  pub amount: Amount,
  pub text: String,
  pub span: FieldSpan,
  /// The anchor score before any reconciliation bonus.
  pub base: f64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Totals {
  pub sub_total: Option<MoneyField>,
  pub total_tax: Option<MoneyField>,
  pub invoice_total: Option<MoneyField>,
  pub amount_due: Option<MoneyField>,
  pub reconciled: bool,
  /// The invoice total or amount due was printed negative.
  pub negative: bool,
}

const BONUS: f64 = 0.2;
const CAP: f64 = 0.99;

fn close(sum: f64, target: f64) -> bool {
  (sum - target).abs() <= 0.01f64.max(0.01 * target.abs())
}

fn money(c: &TotalCandidate, currency: Option<&str>, bonus: f64) -> MoneyField {
  MoneyField {
    value: c.amount.value,
    text: c.text.clone(),
    confidence: (c.base + bonus).min(CAP),
    span: Some(c.span.clone()),
    currency_code: c
      .amount
      .code
      .clone()
      .or_else(|| currency.map(str::to_string)),
    currency_symbol: c.amount.symbol.clone(),
  }
}

fn union(spans: &[&FieldSpan]) -> Option<FieldSpan> {
  let first = *spans.first()?;
  Some(spans.iter().fold(first.clone(), |a, b| FieldSpan {
    page: a.page,
    x0: a.x0.min(b.x0),
    y0: a.y0.min(b.y0),
    x1: a.x1.max(b.x1),
    y1: a.y1.max(b.y1),
  }))
}

fn tax_field(taxes: &[&TotalCandidate], currency: Option<&str>, bonus: f64) -> Option<MoneyField> {
  match taxes {
    [] => None,
    [one] => Some(money(one, currency, bonus)),
    many => Some(MoneyField {
      value: many.iter().map(|t| t.amount.value).sum(),
      text: many
        .iter()
        .map(|t| t.text.as_str())
        .collect::<Vec<_>>()
        .join(" + "),
      confidence: (many.iter().map(|t| t.base).sum::<f64>() / many.len() as f64 + bonus).min(CAP),
      span: union(&many.iter().map(|t| &t.span).collect::<Vec<_>>()),
      currency_code: many[0]
        .amount
        .code
        .clone()
        .or_else(|| currency.map(str::to_string)),
      currency_symbol: many[0].amount.symbol.clone(),
    }),
  }
}

fn best_by_base<'a>(items: &[&'a TotalCandidate]) -> Option<&'a TotalCandidate> {
  items
    .iter()
    .copied()
    .max_by(|a, b| a.base.total_cmp(&b.base))
}

/// Reconcile the candidates. `currency` is the document's code for amounts whose token carries
/// none.
pub(crate) fn resolve(items: &[TotalCandidate], currency: Option<&str>) -> Totals {
  // One amount reached from two labels (`TPS / GST`) is one candidate: keep the best-scored.
  let mut unique: Vec<&TotalCandidate> = Vec::new();
  for c in items {
    match unique
      .iter_mut()
      .find(|u| u.field == c.field && u.span == c.span && u.amount.value == c.amount.value)
    {
      Some(u) => {
        if c.base > u.base {
          *u = c;
        }
      }
      None => unique.push(c),
    }
  }
  let of = |f: Field| -> Vec<&TotalCandidate> {
    unique.iter().copied().filter(|c| c.field == f).collect()
  };
  let subs = of(Field::SubTotal);
  let taxes = of(Field::Tax);
  let totals = of(Field::Total);
  let dues = of(Field::AmountDue);

  let mut tax_sets: Vec<Vec<&TotalCandidate>> = vec![Vec::new()];
  for t in &taxes {
    tax_sets.push(vec![*t]);
  }
  if taxes.len() > 1 {
    tax_sets.push(taxes.clone());
  }

  // (quality, subtotal, total, taxes used, the total is an amount-due line)
  let mut best: Option<(
    f64,
    &TotalCandidate,
    &TotalCandidate,
    Vec<&TotalCandidate>,
    bool,
  )> = None;
  for sub in &subs {
    let total_candidates = totals
      .iter()
      .map(|t| (*t, false))
      .chain(dues.iter().map(|d| (*d, true)));
    for (total, is_due) in total_candidates {
      for set in &tax_sets {
        let tax_sum: f64 = set.iter().map(|t| t.amount.value).sum();
        if !close(sub.amount.value + tax_sum, total.amount.value) {
          continue;
        }
        let quality =
          sub.base + total.base + set.iter().map(|t| t.base).sum::<f64>() + 0.1 * set.len() as f64
            - if is_due { 0.05 } else { 0.0 };
        if best.as_ref().is_none_or(|b| quality > b.0) {
          best = Some((quality, sub, total, set.clone(), is_due));
        }
      }
    }
  }

  if let Some((_, sub, total, set, is_due)) = best {
    let invoice_total = money(total, currency, BONUS);
    let amount_due = if is_due {
      Some(invoice_total.clone())
    } else {
      dues
        .iter()
        .copied()
        .find(|d| close(d.amount.value, total.amount.value))
        .map(|d| money(d, currency, BONUS))
        .or_else(|| best_by_base(&dues).map(|d| money(d, currency, 0.0)))
    };
    let negative = total.amount.negative
      || dues
        .iter()
        .any(|d| close(d.amount.value, total.amount.value) && d.amount.negative);
    return Totals {
      sub_total: Some(money(sub, currency, BONUS)),
      total_tax: tax_field(&set, currency, BONUS),
      invoice_total: Some(invoice_total),
      amount_due,
      reconciled: true,
      negative,
    };
  }

  // No reconciliation: the lowest Total on the page, the largest amount among ties.
  let invoice_total = totals.iter().copied().min_by(|a, b| {
    a.span
      .y0
      .total_cmp(&b.span.y0)
      .then(b.amount.value.total_cmp(&a.amount.value))
  });
  let amount_due = best_by_base(&dues);
  Totals {
    sub_total: best_by_base(&subs).map(|c| money(c, currency, 0.0)),
    total_tax: best_by_base(&taxes).map(|c| money(c, currency, 0.0)),
    invoice_total: invoice_total.map(|c| money(c, currency, 0.0)),
    amount_due: amount_due.map(|c| money(c, currency, 0.0)),
    reconciled: false,
    negative: invoice_total.is_some_and(|c| c.amount.negative)
      || amount_due.is_some_and(|c| c.amount.negative),
  }
}

#[cfg(test)]
mod tests {
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
}
