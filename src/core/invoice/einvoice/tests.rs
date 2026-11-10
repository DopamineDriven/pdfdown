use super::*;
pub(crate) const CII_SAMPLE: &str =
  include_str!("../../../../__test__/fixtures/einvoice/cii-sample.xml");

pub(crate) const UBL_SAMPLE: &str =
  include_str!("../../../../__test__/fixtures/einvoice/ubl-sample.xml");

fn att(name: &str, rel: Option<&str>, data: &str) -> RawAttachment {
  RawAttachment {
    name: name.into(),
    mime_type: Some("text/xml".into()),
    relationship: rel.map(str::to_string),
    description: None,
    page: None,
    data: data.as_bytes().to_vec(),
  }
}

#[test]
fn detects_cii_and_reads_profile() {
  let mut w = Vec::new();
  let found = detect(
    &[att("factur-x.xml", Some("Alternative"), CII_SAMPLE)],
    &mut w,
  )
  .unwrap();
  assert_eq!(found.syntax, XmlSyntax::Cii);
  assert_eq!(found.standard, EInvoiceStandard::FacturX);
  assert!(found.profile.as_deref().unwrap().contains("factur-x.eu"));
  assert!(w.is_empty());
}

#[test]
fn detects_ubl_xrechnung_by_customization_id() {
  let mut w = Vec::new();
  let found = detect(&[att("invoice.xml", None, UBL_SAMPLE)], &mut w).unwrap();
  assert_eq!(found.syntax, XmlSyntax::Ubl);
  assert_eq!(found.standard, EInvoiceStandard::XRechnung);
}

#[test]
fn prefers_relationship_tagged_candidates_and_skips_bad_xml() {
  let mut w = Vec::new();
  let atts = [
    att("notes.xml", None, "<notes><unclosed>"),
    att("readme.txt", None, "plain text"),
    att(
      "zugferd-invoice.xml",
      Some("Data"),
      CII_SAMPLE
        .replace("factur-x.eu:1p0:basic", "zugferd:2p1:basic")
        .as_str(),
    ),
  ];
  let found = detect(&atts, &mut w).unwrap();
  assert_eq!(found.attachment_index, 2);
  assert_eq!(found.standard, EInvoiceStandard::ZUGFeRD);
  assert!(
    w.is_empty(),
    "the tagged candidate wins before the broken one is tried"
  );
}

#[test]
fn broken_xml_is_reported_when_nothing_parses() {
  let mut w = Vec::new();
  assert!(detect(&[att("factur-x.xml", Some("Alternative"), "<x")], &mut w).is_none());
  assert_eq!(w.len(), 1);
  assert!(w[0].contains("not well-formed"));
}

#[test]
fn utf8_bom_is_tolerated() {
  let mut data = vec![0xEF, 0xBB, 0xBF];
  data.extend_from_slice(CII_SAMPLE.as_bytes());
  let a = RawAttachment {
    data,
    ..att("factur-x.xml", Some("Alternative"), "")
  };
  let mut w = Vec::new();
  assert!(detect(&[a], &mut w).is_some());
}

#[test]
fn cii_mapping_covers_header_totals_taxes_payment_and_lines() {
  let doc = roxmltree::Document::parse(CII_SAMPLE).unwrap();
  let (f, kind) = map_cii(doc.root_element());
  assert_eq!(kind, InvoiceDocumentType::Invoice);
  assert_eq!(f.invoice_id.as_ref().unwrap().value, "INV-2026-0042");
  assert_eq!(f.invoice_date.as_ref().unwrap().value, "2026-11-01");
  assert_eq!(f.due_date.as_ref().unwrap().value, "2026-12-01");
  assert_eq!(
    f.vendor_name.as_ref().unwrap().value,
    "Acme Aviation Parts GmbH"
  );
  assert_eq!(
    f.vendor_address.as_ref().unwrap().lines,
    ["Flughafenstrasse 20", "80333 Muenchen", "DE"]
  );
  assert_eq!(
    f.vendor_tax_id.as_ref().unwrap().value,
    "DE123456789",
    "VA scheme wins over FC"
  );
  assert_eq!(
    f.vendor_email.as_ref().unwrap().value,
    "billing@acme-aviation.example"
  );
  assert_eq!(
    f.customer_name.as_ref().unwrap().value,
    "Skyline Charter LLC"
  );
  assert_eq!(f.customer_id.as_ref().unwrap().value, "C-1001");
  assert_eq!(f.purchase_order.as_ref().unwrap().value, "PO-7788");
  assert_eq!(f.payment_term.as_ref().unwrap().value, "Net 30");
  assert_eq!(f.currency_code.as_deref(), Some("EUR"));
  assert_eq!(f.sub_total.as_ref().unwrap().value, 700.0);
  assert_eq!(f.total_tax.as_ref().unwrap().value, 133.0);
  assert_eq!(f.invoice_total.as_ref().unwrap().value, 833.0);
  assert_eq!(f.invoice_total.as_ref().unwrap().text, "833.00");
  assert_eq!(
    f.invoice_total.as_ref().unwrap().currency_code.as_deref(),
    Some("EUR")
  );
  assert_eq!(f.amount_due.as_ref().unwrap().value, 833.0);
  assert!(
    f.previous_unpaid_balance.is_none(),
    "zero prepaid is dropped"
  );
  assert_eq!(f.tax_details.len(), 1);
  assert_eq!(f.tax_details[0].rate.as_ref().unwrap().value, 19.0);
  assert_eq!(f.tax_details[0].base.as_ref().unwrap().value, 700.0);
  assert_eq!(f.tax_details[0].category.as_ref().unwrap().value, "S");
  assert_eq!(f.payment_details.len(), 1);
  assert_eq!(
    f.payment_details[0].iban.as_ref().unwrap().value,
    "DE89370400440532013000"
  );
  assert_eq!(
    f.payment_details[0].swift.as_ref().unwrap().value,
    "COBADEFFXXX"
  );
  assert_eq!(f.items.len(), 2);
  let first = &f.items[0];
  assert_eq!(
    first.description.as_ref().unwrap().value,
    "Hydraulic pump seal kit"
  );
  assert_eq!(first.product_code.as_ref().unwrap().value, "HPS-220");
  assert_eq!(first.quantity.as_ref().unwrap().value, 4.0);
  assert_eq!(first.unit.as_ref().unwrap().value, "H87");
  assert_eq!(first.unit_price.as_ref().unwrap().value, 125.0);
  assert_eq!(first.tax_rate.as_ref().unwrap().value, 19.0);
  assert_eq!(first.amount.as_ref().unwrap().value, 500.0);
  assert_eq!(f.items[1].quantity.as_ref().unwrap().value, 2.5);
  assert!(f.items[1].product_code.is_none());
  assert!(f.items.iter().all(|i| i.span.is_none()));
  assert!((f.invoice_total.as_ref().unwrap().confidence - 1.0).abs() < f64::EPSILON);
}

#[test]
fn ubl_mapping_covers_parties_totals_taxes_payment_and_lines() {
  let doc = roxmltree::Document::parse(UBL_SAMPLE).unwrap();
  let (f, kind) = map_ubl(doc.root_element());
  assert_eq!(kind, InvoiceDocumentType::Invoice);
  assert_eq!(f.invoice_id.as_ref().unwrap().value, "UBL-77");
  assert_eq!(f.invoice_date.as_ref().unwrap().value, "2026-10-15");
  assert_eq!(f.due_date.as_ref().unwrap().value, "2026-11-14");
  assert_eq!(f.currency_code.as_deref(), Some("USD"));
  assert_eq!(f.purchase_order.as_ref().unwrap().value, "PO-9");
  assert_eq!(f.vendor_name.as_ref().unwrap().value, "Sliced Aero LLC");
  assert_eq!(
    f.vendor_address.as_ref().unwrap().lines,
    ["1 Hangar Way", "67209 Wichita", "US"]
  );
  assert_eq!(f.vendor_tax_id.as_ref().unwrap().value, "US12-3456789");
  assert_eq!(
    f.vendor_email.as_ref().unwrap().value,
    "ar@slicedaero.example"
  );
  assert_eq!(f.vendor_phone.as_ref().unwrap().value, "+1 316 555 0100");
  assert_eq!(f.customer_name.as_ref().unwrap().value, "Skyline Charter");
  assert_eq!(f.customer_id.as_ref().unwrap().value, "SKY-01");
  assert_eq!(
    f.payment_details[0].iban.as_ref().unwrap().value,
    "US64SVBKUS6S3300958879"
  );
  assert_eq!(
    f.payment_details[0].swift.as_ref().unwrap().value,
    "SVBKUS6S"
  );
  assert_eq!(
    f.payment_details[0].bank_name.as_ref().unwrap().value,
    "Sliced Aero Operating"
  );
  assert_eq!(f.payment_term.as_ref().unwrap().value, "Net 30");
  assert_eq!(f.tax_details.len(), 1);
  assert_eq!(f.tax_details[0].rate.as_ref().unwrap().value, 8.0);
  assert_eq!(f.total_tax.as_ref().unwrap().value, 8.0);
  assert_eq!(f.sub_total.as_ref().unwrap().value, 100.0);
  assert_eq!(f.invoice_total.as_ref().unwrap().value, 108.0);
  assert_eq!(f.amount_due.as_ref().unwrap().value, 108.0);
  assert_eq!(f.items.len(), 1);
  assert_eq!(f.items[0].description.as_ref().unwrap().value, "Widget");
  assert_eq!(f.items[0].product_code.as_ref().unwrap().value, "W-1");
  assert_eq!(f.items[0].quantity.as_ref().unwrap().value, 10.0);
  assert_eq!(f.items[0].unit.as_ref().unwrap().value, "EA");
  assert_eq!(f.items[0].unit_price.as_ref().unwrap().value, 10.0);
  assert_eq!(f.items[0].amount.as_ref().unwrap().value, 100.0);
  assert_eq!(f.items[0].tax_rate.as_ref().unwrap().value, 8.0);
}

#[test]
fn credit_notes_are_recognised_in_both_syntaxes() {
  let cii = CII_SAMPLE.replace(
    "<ram:TypeCode>380</ram:TypeCode>",
    "<ram:TypeCode>381</ram:TypeCode>",
  );
  let doc = roxmltree::Document::parse(&cii).unwrap();
  assert_eq!(
    map_cii(doc.root_element()).1,
    InvoiceDocumentType::CreditNote
  );
  let ubl = UBL_SAMPLE
    .replace(
      "<Invoice xmlns=\"urn:oasis:names:specification:ubl:schema:xsd:Invoice-2\"",
      "<CreditNote xmlns=\"urn:oasis:names:specification:ubl:schema:xsd:CreditNote-2\"",
    )
    .replace("</Invoice>", "</CreditNote>")
    .replace("InvoiceLine", "CreditNoteLine")
    .replace("InvoicedQuantity", "CreditedQuantity");
  let doc = roxmltree::Document::parse(&ubl).unwrap();
  let (f, kind) = map_ubl(doc.root_element());
  assert_eq!(kind, InvoiceDocumentType::CreditNote);
  assert_eq!(f.items.len(), 1);
  assert_eq!(f.items[0].quantity.as_ref().unwrap().value, 10.0);
}

#[test]
fn dates_and_numbers_parse_defensively() {
  assert_eq!(date_from_text("20261101"), Some("2026-11-01".into()));
  assert_eq!(
    date_from_text("2026-11-01T10:00:00"),
    Some("2026-11-01".into())
  );
  assert_eq!(date_from_text("tomorrow"), None);
  assert_eq!(
    nf("1.234,56".into()).map(|n| n.value),
    None,
    "ambiguous separators are not guessed"
  );
  assert_eq!(nf("12,5".into()).map(|n| n.value), Some(12.5));
  assert!(mf("abc".into(), None).is_none());
}
