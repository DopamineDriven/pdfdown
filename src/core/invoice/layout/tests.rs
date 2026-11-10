use super::super::testing::acme_page;
use super::*;
use crate::types::CurrencySource;

#[test]
fn repeated_page_numbers_never_abort_the_host() {
  let mut empty = acme_page();
  empty.words.clear();
  empty.lines.clear();
  let r = extract_from_pages(&[empty, acme_page()], None);
  assert_eq!(r.fields.invoice_id.unwrap().value, "A-10045");
  assert!(
    r.warnings.iter().any(|w| w.contains("page number")),
    "{:?}",
    r.warnings
  );
}

#[test]
fn everyday_lines_do_not_fabricate_fields() {
  use super::super::testing::page;
  let r = extract_from_pages(
    &[page(&[
      (50.0, 760.0, 14.0, true, "ACME AVIATION SERVICES"),
      (380.0, 744.0, 10.0, false, "Due Date: 01/31/2016"),
      (380.0, 730.0, 10.0, false, "Invoice Date: 01/25/2016"),
      (380.0, 716.0, 10.0, false, "Invoice No: A-1"),
      (380.0, 702.0, 10.0, false, "Vendor #: 4471"),
      (380.0, 688.0, 10.0, false, "Customer No: 10045"),
      (50.0, 660.0, 10.0, false, "Terms: Net 30"),
      (50.0, 640.0, 10.0, false, "Total hours worked on site 12"),
      (400.0, 300.0, 10.0, false, "Total"),
      (500.0, 300.0, 10.0, false, "$93.50"),
      (400.0, 286.0, 10.0, false, "Amount Due"),
      (500.0, 286.0, 10.0, false, "-$12.50"),
    ])],
    None,
  );
  assert_eq!(r.fields.invoice_date.unwrap().value, "2016-01-25");
  assert_eq!(r.fields.due_date.unwrap().value, "2016-01-31");
  assert_eq!(
    r.fields.vendor_name.unwrap().value,
    "ACME AVIATION SERVICES"
  );
  assert_eq!(r.fields.customer_id.unwrap().value, "10045");
  assert!(r.fields.customer_name.is_none());
  assert!(
    r.fields.sub_total.is_none(),
    "`Net 30` is terms, not a subtotal"
  );
  assert_eq!(
    r.fields.invoice_total.unwrap().value,
    93.5,
    "prose with a trailing number is not a total"
  );
  assert_eq!(
    r.document_type,
    InvoiceDocumentType::Invoice,
    "an overpaid balance is not a credit note"
  );
  assert!(r.credit_signals.is_empty());
}

#[test]
fn a_bare_title_anchors_nothing_below_or_beside_it() {
  use super::super::testing::page;
  let r = extract_from_pages(
    &[page(&[
      (50.0, 760.0, 12.0, true, "INVOICE"),
      (50.0, 744.0, 10.0, false, "1200 Hangar Road"),
      (300.0, 760.0, 10.0, false, "03/14/2024"),
      (50.0, 700.0, 14.0, true, "ACME AVIATION SERVICES"),
    ])],
    None,
  );
  assert!(r.fields.invoice_id.is_none(), "{:?}", r.fields.invoice_id);
  assert!(r.fields.total_tax.is_none());
  let r = extract_from_pages(
    &[page(&[
      (50.0, 760.0, 12.0, true, "TAX INVOICE"),
      (50.0, 744.0, 10.0, false, "123 Main Street"),
    ])],
    None,
  );
  assert!(r.fields.invoice_id.is_none());
  assert!(r.fields.total_tax.is_none());
}

#[test]
fn a_two_thousand_word_line_is_read_in_well_under_a_second() {
  use super::super::testing::page;
  let text: String = (0..400)
    .map(|i| format!("word{i} alpha beta Date gamma"))
    .collect::<Vec<_>>()
    .join(" ");
  let p = page(&[(10.0, 700.0, 10.0, false, &text)]);
  let started = std::time::Instant::now();
  let _ = extract_from_pages(&[p], None);
  assert!(
    started.elapsed().as_millis() < 1000,
    "{} ms",
    started.elapsed().as_millis()
  );
}

#[test]
fn a_cover_page_summary_outranks_the_itemisation_pages() {
  use super::super::testing::page;
  // A cover page: a three-column summary (Time & Materials, Flat Rate, Total) whose totals
  // labels sit left of three amounts, then a "Total Amount" line; the last page is a job card
  // with its own section total and the only "Bill To" line.
  let cover = page(&[
    (50.0, 760.0, 14.0, true, "INVOICE 2048811"),
    (330.0, 700.0, 10.0, false, "Invoice Date: 11/06/2026"),
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
    (81.0, 210.0, 10.0, false, "Tax:"),
    (191.0, 210.0, 10.0, false, "$0.00"),
    (299.0, 210.0, 10.0, false, "$0.00"),
    (407.0, 210.0, 10.0, false, "$0.00"),
    (270.0, 172.0, 10.0, false, "Total Amount:"),
    (411.0, 172.0, 10.0, false, "$1,500.00"),
  ]);
  let mut last = page(&[
    (5.0, 661.0, 10.0, false, "Bill To: Skyline Charter LLC"),
    (448.0, 661.0, 10.0, false, "Work Order: 771234"),
    (49.0, 613.0, 10.0, false, "Labor"),
    (400.0, 498.0, 10.0, false, "Total:"),
    (533.0, 498.0, 10.0, false, "$412.80"),
  ]);
  last.page = 2;
  let r = extract_from_pages(&[cover, last], None);
  let f = &r.fields;
  assert_eq!(f.invoice_id.as_ref().unwrap().value, "2048811");
  assert_eq!(
    f.sub_total.as_ref().unwrap().value,
    1500.0,
    "the last column of the summary, not the first"
  );
  let total = f.invoice_total.as_ref().unwrap();
  assert_eq!(
    total.value, 1500.0,
    "the cover page's total, not the last job card's"
  );
  assert_eq!(total.span.as_ref().unwrap().page, 1);
  assert_eq!(
    f.customer_name.as_ref().unwrap().value,
    "Skyline Charter LLC",
    "Bill To from the last page when the cover has none"
  );
  assert_eq!(
    f.customer_name
      .as_ref()
      .unwrap()
      .span
      .as_ref()
      .unwrap()
      .page,
    2
  );
}

#[test]
fn a_spanish_quote_reads_through_the_spanish_rows() {
  use super::super::testing::page;
  // A Latin American estimate: the title row carries the number, the customer and the
  // aircraft sit under one-word labels, and the amounts are written `US$ 400.00`.
  let p = page(&[
    (470.0, 760.0, 11.0, true, "PRESUPUESTO: F-10001"),
    (640.0, 760.0, 9.0, false, "FECHA: 23/07/2014"),
    (640.0, 720.0, 9.0, false, "SIGLAS: N123AB"),
    (640.0, 706.0, 9.0, false, "CLIENTE: Skyline Charter"),
    (60.0, 600.0, 9.0, false, "SERIAL MOTOR: P-100200"),
    (60.0, 586.0, 9.0, false, "MOTOR TIPO: TPE-9000"),
    (60.0, 572.0, 9.0, false, "HORAS: 4.00"),
    (420.0, 300.0, 9.0, false, "Sub-Total ...."),
    (560.0, 300.0, 9.0, false, "US$"),
    (620.0, 300.0, 9.0, false, "400.00"),
    (420.0, 260.0, 9.0, false, "TOTAL PRESUPUESTO:"),
    (560.0, 260.0, 9.0, false, "US$"),
    (620.0, 260.0, 9.0, false, "400.00"),
  ]);
  let r = extract_from_pages(&[p], None);
  let f = &r.fields;
  assert_eq!(
    f.invoice_id.as_ref().unwrap().value,
    "F-10001",
    "{:?}",
    f.invoice_id
  );
  assert_eq!(f.invoice_date.as_ref().unwrap().value, "2014-07-23");
  assert_eq!(f.customer_name.as_ref().unwrap().value, "Skyline Charter");
  assert_eq!(f.aircraft[0].registration.as_ref().unwrap().value, "N123AB");
  assert_eq!(
    f.aircraft[0].engine_serial_number.as_ref().unwrap().value,
    "P-100200"
  );
  assert_eq!(f.sub_total.as_ref().unwrap().value, 400.0);
  assert_eq!(f.invoice_total.as_ref().unwrap().value, 400.0);
  assert_eq!(f.currency_code.as_deref(), Some("USD"));
  assert!(
    f.aircraft[0].serial_number.is_none(),
    "`Serial` inside `Serial Motor` is not the aircraft serial: {:?}",
    f.aircraft[0].serial_number
  );
}

fn purchase_order_page() -> PageLayout {
  use super::super::testing::page;
  let mut p = page(&[
    (50.0, 760.0, 14.0, true, "PURCHASE ORDER"),
    (380.0, 730.0, 10.0, false, "P.O. Number: PO-990011"),
    (380.0, 716.0, 10.0, false, "Date: 05/01/2024"),
    (50.0, 690.0, 10.0, false, "Ship To:"),
    (50.0, 676.0, 10.0, false, "Skyline Charter LLC"),
    (400.0, 300.0, 10.0, false, "Total"),
    (500.0, 300.0, 10.0, false, "$99.00"),
  ]);
  p.page = 2;
  p
}

#[test]
fn an_appended_purchase_order_page_is_not_the_invoice() {
  use super::super::testing::{acme_page, page};
  let alone = extract_from_pages(&[acme_page()], None);
  let with_po = extract_from_pages(&[acme_page(), purchase_order_page()], None);
  let f = &with_po.fields;
  assert_eq!(f.invoice_id.as_ref().unwrap().value, "A-10045");
  assert_eq!(f.invoice_date.as_ref().unwrap().value, "2024-03-14");
  assert_eq!(
    f.invoice_total.as_ref().unwrap().value,
    alone.fields.invoice_total.as_ref().unwrap().value,
    "the invoice's total, not the purchase order's"
  );
  assert_eq!(
    f.purchase_order.as_ref().unwrap().value,
    "PO-778812",
    "the invoice's own PO line outranks the appended order"
  );
  assert!(
    with_po
      .warnings
      .iter()
      .any(|w| w.contains("page 2 is a purchase order")),
    "{:?}",
    with_po.warnings
  );

  // Without a PO line on the invoice, the appended order supplies its number.
  let mut bare = page(&[
    (450.0, 760.0, 12.0, true, "INVOICE"),
    (380.0, 730.0, 10.0, false, "Invoice No: A-10046"),
    (380.0, 716.0, 10.0, false, "Invoice Date: 03/14/2024"),
    (400.0, 300.0, 10.0, false, "Total Due"),
    (500.0, 300.0, 10.0, false, "$50.00"),
  ]);
  bare.page = 1;
  let r = extract_from_pages(&[bare, purchase_order_page()], None);
  assert_eq!(r.fields.invoice_id.as_ref().unwrap().value, "A-10046");
  assert_eq!(r.fields.purchase_order.as_ref().unwrap().value, "PO-990011");
  assert_eq!(
    r.fields
      .purchase_order
      .as_ref()
      .unwrap()
      .span
      .as_ref()
      .unwrap()
      .page,
    2
  );
  assert_eq!(r.fields.invoice_total.as_ref().unwrap().value, 50.0);
}

#[test]
fn a_purchase_order_alone_is_read_as_it_stands_with_a_warning() {
  let r = extract_from_pages(&[purchase_order_page()], None);
  assert_eq!(r.fields.purchase_order.as_ref().unwrap().value, "PO-990011");
  assert_eq!(r.fields.invoice_total.as_ref().unwrap().value, 99.0);
  assert!(
    r.warnings
      .iter()
      .any(|w| w.contains("titled as a purchase order")),
    "{:?}",
    r.warnings
  );
}

#[test]
fn a_boxed_form_header_cell_is_not_a_page_title() {
  use super::super::testing::page;
  // A one-page invoice whose boxed header row has a `PURCHASE ORDER` cell in body-sized text.
  let p = page(&[
    (50.0, 760.0, 14.0, true, "ACME AVIATION SERVICES"),
    (50.0, 700.0, 10.0, false, "PURCHASE ORDER"),
    (200.0, 700.0, 10.0, false, "DATE"),
    (350.0, 700.0, 10.0, false, "TERMS"),
    (50.0, 686.0, 10.0, false, "PO-778812"),
    (200.0, 686.0, 10.0, false, "03/14/2024"),
    (350.0, 686.0, 10.0, false, "Net 30"),
    (380.0, 640.0, 10.0, false, "Invoice No: A-10045"),
    (400.0, 300.0, 10.0, false, "Total Due"),
    (500.0, 300.0, 10.0, false, "$50.00"),
  ]);
  let r = extract_from_pages(&[p], None);
  assert_eq!(r.fields.invoice_id.as_ref().unwrap().value, "A-10045");
  assert_eq!(r.fields.invoice_total.as_ref().unwrap().value, 50.0);
  assert!(
    !r.warnings.iter().any(|w| w.contains("purchase order")),
    "{:?}",
    r.warnings
  );
}

#[test]
fn the_header_fields_come_off_the_acme_page() {
  let r = extract_from_pages(&[acme_page()], None);
  let id = r.fields.invoice_id.expect("invoice id");
  assert_eq!(id.value, "A-10045");
  assert_eq!(id.span.unwrap().page, 1);
  assert!(id.confidence >= 0.8, "{}", id.confidence);
  assert_eq!(r.fields.invoice_date.unwrap().value, "2024-03-14");
  assert_eq!(r.fields.purchase_order.unwrap().value, "PO-778812");
  assert_eq!(r.fields.work_order.unwrap().value, "WO-5521");
  assert_eq!(
    r.fields.vendor_name.unwrap().value,
    "ACME AVIATION SERVICES"
  );
  assert_eq!(r.fields.customer_name.unwrap().value, "Skyline Charter LLC");
  assert_eq!(r.fields.aircraft.len(), 1);
  assert_eq!(
    r.fields.aircraft[0].registration.as_ref().unwrap().value,
    "N417SK"
  );
  assert_eq!(
    r.fields.aircraft[0].serial_number.as_ref().unwrap().value,
    "20311"
  );
  assert_eq!(r.currency_source, Some(CurrencySource::Unknown));
  assert!(!r.warnings.iter().any(|w| w.contains("not implemented")));
}
