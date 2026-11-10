use super::*;

#[test]
fn normalisation_folds_case_punctuation_and_ocr_confusables() {
  assert_eq!(normalize("lnvoice Nο."), normalize("Invoice No."));
  assert_eq!(normalize("P.O. Number:"), normalize("po number"));
  assert_eq!(
    normalize("TOTAL"),
    normalize("Total"),
    "case folds before confusables"
  );
  assert_eq!(normalize("BILL TO:"), normalize("Bill To"));
  assert_eq!(normalize("CLIENTE:"), normalize("Cliente"));
  assert_eq!(
    find_labels(&["BILL", "TO:", "Skyline"], Some(Field::BillTo))[0].score,
    1.0
  );
  assert_eq!(
    find_labels(&["TOTAL", "$93.50"], Some(Field::Total))[0].score,
    1.0
  );
}

#[test]
fn labels_hit_at_the_start_of_a_segment_and_before_a_separator() {
  let hits = find_labels(&["Invoice", "#13594027"], Some(Field::InvoiceId));
  assert_eq!(hits[0].label.text, "Invoice #");
  assert_eq!(
    hits[0].end, 2,
    "the glued `#13594027` word belongs to the label match"
  );
  assert_eq!(hits[0].remainder, "13594027");

  let hits = find_labels(&["Rechnung", "Nr.", "2015-0831"], None);
  assert_eq!(hits[0].label.lang, "de");
  assert_eq!(hits[0].label.field, Field::InvoiceId);
  assert_eq!(hits[0].remainder, "2015-0831");

  let due = find_labels(&["Total", "Due", "$93.50"], Some(Field::AmountDue));
  assert_eq!(due[0].score, 1.0);
  assert_eq!(due[0].remainder, "$93.50");
  let total = find_labels(&["Total", "Due", "$93.50"], Some(Field::Total));
  assert_eq!(
    total[0].score, 0.8,
    "`Total` is a prefix of the segment, not the whole label"
  );

  assert!(find_labels(&["Terms:", "Net", "30"], Some(Field::InvoiceDate)).is_empty());
  assert_eq!(
    find_labels(&["Invoice", "Number", "INV-3337"], Some(Field::InvoiceId))[0].remainder,
    "INV-3337"
  );
}

#[test]
fn a_label_inside_a_longer_label_of_another_field_never_hits() {
  assert!(
    find_labels(&["Due", "Date:", "01/31/2016"], Some(Field::InvoiceDate)).is_empty(),
    "`Date` inside `Due Date`"
  );
  assert!(find_labels(&["Ship", "Date:", "03/10/2024"], Some(Field::InvoiceDate)).is_empty());
  assert!(
    find_labels(
      &["Sales", "Order", "No.", "55123"],
      Some(Field::PurchaseOrder)
    )
    .is_empty(),
    "`Order No.` inside `Sales Order`"
  );
  assert_eq!(
    find_labels(&["Sales", "Order", "No.", "55123"], Some(Field::WorkOrder))[0].remainder,
    "55123"
  );
  assert!(
    find_labels(&["Terms:", "Net", "30"], Some(Field::SubTotal)).is_empty(),
    "a terms line labels nothing"
  );
  assert!(
    find_labels(&["Sub", "Total", "$85.00"], Some(Field::Total)).is_empty(),
    "`Total` inside `Sub Total`"
  );
  assert!(
    find_labels(&["TAX", "INVOICE"], Some(Field::Tax)).is_empty(),
    "`Tax` inside the title"
  );
  assert!(
    find_labels(&["Previous", "Balance", "$120.00"], Some(Field::AmountDue)).is_empty(),
    "`Balance` inside a label of something this reader does not extract"
  );
  assert!(
    find_labels(
      &["Date", "Shipped:", "03/10/2024"],
      Some(Field::InvoiceDate)
    )
    .is_empty(),
    "`Date` opening such a label"
  );
  assert!(
    find_labels(
      &["Customer", "Project", "Manager:", "Avery", "Stone"],
      Some(Field::BillTo)
    )
    .is_empty(),
    "`Customer` opening a contact label"
  );
  assert_eq!(
    find_labels(
      &["Statement", "Date:", "03/10/2024"],
      Some(Field::InvoiceDate)
    )[0]
      .score,
    0.8,
    "an unknown qualifier only lowers the score"
  );
  assert_eq!(
    find_labels(&["Balance", "Due", "$93.50"], Some(Field::AmountDue))[0].score,
    1.0
  );
  let customer = find_labels(
    &["Customer", "Invoice", "Number:", "INV-7"],
    Some(Field::InvoiceId),
  );
  assert_eq!(
    customer[0].score, 0.8,
    "a label that does not open its segment scores lower"
  );
  assert_eq!(customer[0].remainder, "INV-7");
  assert_eq!(
    find_labels(&["Terms:", "Net", "30"], None)[0].label.field,
    Field::Terms,
    "a terms line stays labelled as terms"
  );
  let bilingual = find_labels(
    &["No", "de", "facture", "/", "Invoice", "No.:", "F-2209"],
    Some(Field::InvoiceId),
  );
  assert!(
    bilingual.iter().any(|h| h.remainder == "F-2209"),
    "a second label of the same field survives"
  );
}

#[test]
fn page_roles_come_from_a_bare_title() {
  assert_eq!(
    role_of_title(&["PURCHASE", "ORDER"]),
    Some(PageRole::PurchaseOrder)
  );
  assert_eq!(
    role_of_title(&["Packing", "Slip"]),
    Some(PageRole::Delivery)
  );
  assert_eq!(role_of_title(&["Lieferschein"]), Some(PageRole::Delivery));
  assert_eq!(role_of_title(&["INVOICE"]), Some(PageRole::Invoice));
  assert_eq!(role_of_title(&["FACTURA"]), Some(PageRole::Invoice));
  assert_eq!(
    role_of_title(&["Purchase", "Order:", "249604"]),
    None,
    "a label with a value is not a title"
  );
  assert_eq!(role_of_title(&["ACME", "AVIATION"]), None);
}

#[test]
fn document_titles_are_known() {
  assert!(is_document_title(&["INVOICE"]));
  assert!(is_document_title(&["Maintenance", "Reimbursement", "Form"]));
  assert!(is_document_title(&["FACTURE", "/", "INVOICE"]));
  assert!(is_document_title(&["PRESUPUESTO"]));
  let reg = find_labels(&["Aircraft", "Number:", "N92"], Some(Field::Registration));
  assert_eq!(
    reg[0].remainder, "N92",
    "a fleet-style label still names the registration"
  );
  assert!(is_document_title(&["Devis"]));
  assert!(!is_document_title(&["ACME", "AVIATION", "SERVICES"]));
}
