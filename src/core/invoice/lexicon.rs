//! The label vocabulary: a reviewed table of spellings per field, each tagged with its language,
//! seeded from the service's retired parsers (1037.md §3.3) and the design (2026-11-01 §7.3).
//! Matching is on a normalised form that folds case, punctuation and OCR confusables, and is
//! scored exact > prefix > fuzzy. The table carries spellings, never where they were seen.

/// What a label names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Field {
  InvoiceId,
  InvoiceDate,
  DueDate,
  PurchaseOrder,
  BillTo,
  ShipTo,
  RemitTo,
  Vendor,
  SubTotal,
  Tax,
  Total,
  AmountDue,
  Terms,
  WorkOrder,
  Registration,
  Serial,
  EngineSerial,
  CreditTitle,
  DocumentTitle,
  /// A label of something this reader does not extract, listed so its words are not read as
  /// another field's label (`Date` in `Ship Date`, `Balance` in `Previous Balance`).
  Other,
}

#[derive(Debug)]
pub(crate) struct Label {
  pub field: Field,
  pub text: &'static str,
  pub lang: &'static str,
}

const fn l(field: Field, text: &'static str, lang: &'static str) -> Label {
  Label { field, text, lang }
}

use Field::*;

pub(crate) const LABELS: &[Label] = &[
  // invoice number
  l(InvoiceId, "Invoice No", "en"),
  l(InvoiceId, "Invoice No.", "en"),
  l(InvoiceId, "Invoice Number", "en"),
  l(InvoiceId, "Invoice #", "en"),
  l(InvoiceId, "Invoice", "en"),
  l(InvoiceId, "Invoice Nr.", "en"),
  l(InvoiceId, "Invoice N°", "en"),
  l(InvoiceId, "Invoice Nbr", "en"),
  l(InvoiceId, "Tax Invoice No.", "en"),
  l(InvoiceId, "Customer Invoice #", "en"),
  l(InvoiceId, "Credit Memo #", "en"),
  l(InvoiceId, "Credit Memo No", "en"),
  l(InvoiceId, "Credit Note No", "en"),
  l(InvoiceId, "Credit Note #", "en"),
  l(InvoiceId, "Document No.", "en"),
  l(InvoiceId, "Billing #", "en"),
  l(InvoiceId, "Rechnung Nr.", "de"),
  l(InvoiceId, "Rechnungsnummer", "de"),
  l(InvoiceId, "Rechnungs-Nr.", "de"),
  l(InvoiceId, "Facture No", "fr"),
  l(InvoiceId, "Facture N°", "fr"),
  l(InvoiceId, "No de facture", "fr"),
  l(InvoiceId, "Numéro de facture", "fr"),
  l(InvoiceId, "Factura No", "es"),
  l(InvoiceId, "Factura No.", "es"),
  l(InvoiceId, "Factura Nº", "es"),
  l(InvoiceId, "Factura #", "es"),
  l(InvoiceId, "Factura", "es"),
  l(InvoiceId, "Número de factura", "es"),
  l(InvoiceId, "No. de factura", "es"),
  l(InvoiceId, "Nº de factura", "es"),
  l(InvoiceId, "Folio", "es"),
  l(InvoiceId, "Presupuesto", "es"),
  l(InvoiceId, "Presupuesto No.", "es"),
  l(InvoiceId, "Cotización", "es"),
  l(InvoiceId, "Cotización No.", "es"),
  l(InvoiceId, "Facture", "fr"),
  l(InvoiceId, "Devis", "fr"),
  l(InvoiceId, "Devis No", "fr"),
  l(InvoiceId, "Rechnung", "de"),
  l(InvoiceId, "Angebot", "de"),
  l(InvoiceId, "Angebotsnummer", "de"),
  // invoice date
  l(InvoiceDate, "Invoice Date", "en"),
  l(InvoiceDate, "Date", "en"),
  l(InvoiceDate, "Date Issued", "en"),
  l(InvoiceDate, "Issue Date", "en"),
  l(InvoiceDate, "Date of Issue", "en"),
  l(InvoiceDate, "Document Date", "en"),
  l(InvoiceDate, "Billing Date", "en"),
  l(InvoiceDate, "Inv. Date", "en"),
  l(InvoiceDate, "Tax Point", "en"),
  l(InvoiceDate, "Datum", "de"),
  l(InvoiceDate, "Rechnungsdatum", "de"),
  l(InvoiceDate, "Date de facture", "fr"),
  l(InvoiceDate, "Fecha", "es"),
  l(InvoiceDate, "Fecha de factura", "es"),
  l(InvoiceDate, "Fecha factura", "es"),
  l(InvoiceDate, "Fecha de emisión", "es"),
  l(InvoiceDate, "Fecha de expedición", "es"),
  l(InvoiceDate, "Date", "fr"),
  l(InvoiceDate, "Date d'émission", "fr"),
  l(InvoiceDate, "Belegdatum", "de"),
  l(InvoiceDate, "Ausstellungsdatum", "de"),
  // due date
  l(DueDate, "Due Date", "en"),
  l(DueDate, "Payment Due", "en"),
  l(DueDate, "Pay By", "en"),
  l(DueDate, "Due", "en"),
  l(DueDate, "Fälligkeit", "de"),
  l(DueDate, "Fällig am", "de"),
  l(DueDate, "Échéance", "fr"),
  l(DueDate, "Fecha de vencimiento", "es"),
  l(DueDate, "Vencimiento", "es"),
  l(DueDate, "Vence", "es"),
  l(DueDate, "Fecha límite de pago", "es"),
  l(DueDate, "Fälligkeitsdatum", "de"),
  l(DueDate, "Zahlbar bis", "de"),
  l(DueDate, "Date d'échéance", "fr"),
  // purchase order
  l(PurchaseOrder, "Purchase Order", "en"),
  l(PurchaseOrder, "Purchase Order No", "en"),
  l(PurchaseOrder, "Purchase Order Number", "en"),
  l(PurchaseOrder, "P.O. Number", "en"),
  l(PurchaseOrder, "P.O. No.", "en"),
  l(PurchaseOrder, "PO Number", "en"),
  l(PurchaseOrder, "PO#", "en"),
  l(PurchaseOrder, "PO #", "en"),
  l(PurchaseOrder, "PO No.", "en"),
  l(PurchaseOrder, "PO", "en"),
  l(PurchaseOrder, "Customer PO", "en"),
  l(PurchaseOrder, "Cust PO#", "en"),
  l(PurchaseOrder, "Cust. Ref.", "en"),
  l(PurchaseOrder, "Order Number", "en"),
  l(PurchaseOrder, "Order No.", "en"),
  l(PurchaseOrder, "Your Ref", "en"),
  l(PurchaseOrder, "Your Ref.", "en"),
  l(PurchaseOrder, "Y/Ref", "en"),
  l(PurchaseOrder, "Customer Reference", "en"),
  l(PurchaseOrder, "Customer Order No", "en"),
  l(PurchaseOrder, "PO Reference", "en"),
  l(PurchaseOrder, "Bestellnummer", "de"),
  l(PurchaseOrder, "Bestell-Nr.", "de"),
  l(PurchaseOrder, "Bon de commande", "fr"),
  l(PurchaseOrder, "Orden de compra", "es"),
  l(PurchaseOrder, "Orden de compra No.", "es"),
  l(PurchaseOrder, "No. de orden", "es"),
  l(PurchaseOrder, "Pedido", "es"),
  l(PurchaseOrder, "Su pedido", "es"),
  l(PurchaseOrder, "No. de pedido", "es"),
  l(PurchaseOrder, "Su referencia", "es"),
  l(PurchaseOrder, "Votre référence", "fr"),
  l(PurchaseOrder, "Ihre Bestellung", "de"),
  l(PurchaseOrder, "Ihre Referenz", "de"),
  l(PurchaseOrder, "No de commande", "fr"),
  // parties
  l(BillTo, "Bill To", "en"),
  l(BillTo, "Billed To", "en"),
  l(BillTo, "Invoice To", "en"),
  l(BillTo, "Invoiced To", "en"),
  l(BillTo, "Sold To", "en"),
  l(BillTo, "Customer", "en"),
  l(BillTo, "To", "en"),
  l(BillTo, "Rechnung an", "de"),
  l(BillTo, "Kunde", "de"),
  l(BillTo, "Facturé à", "fr"),
  l(BillTo, "Client", "fr"),
  l(BillTo, "Cliente", "es"),
  l(BillTo, "Señores", "es"),
  l(BillTo, "Sres.", "es"),
  l(BillTo, "Facturar a", "es"),
  l(BillTo, "Datos del cliente", "es"),
  l(BillTo, "Razón social", "es"),
  l(ShipTo, "Ship To", "en"),
  l(ShipTo, "Deliver To", "en"),
  l(ShipTo, "Delivery Address", "en"),
  l(ShipTo, "Lieferadresse", "de"),
  l(ShipTo, "Livré à", "fr"),
  l(ShipTo, "Enviar a", "es"),
  l(ShipTo, "Entregar a", "es"),
  l(ShipTo, "Dirección de entrega", "es"),
  l(ShipTo, "Adresse de livraison", "fr"),
  l(ShipTo, "Versandadresse", "de"),
  l(RemitTo, "Remit To", "en"),
  l(RemitTo, "Pay To", "en"),
  l(RemitTo, "Remittance", "en"),
  l(Vendor, "From", "en"),
  l(Vendor, "Vendor", "en"),
  l(Vendor, "Supplier", "en"),
  l(Vendor, "Seller", "en"),
  // totals
  l(SubTotal, "Sub Total", "en"),
  l(SubTotal, "Subtotal", "en"),
  l(SubTotal, "Sub-Total", "en"),
  l(SubTotal, "Net", "en"),
  l(SubTotal, "Net Amount", "en"),
  l(SubTotal, "Total Excluding VAT", "en"),
  l(SubTotal, "Total Excluding Tax", "en"),
  l(SubTotal, "Zwischensumme", "de"),
  l(SubTotal, "Nettobetrag", "de"),
  l(SubTotal, "Netto", "de"),
  l(SubTotal, "Sous-total", "fr"),
  l(SubTotal, "Montant HT", "fr"),
  l(SubTotal, "Total avant taxes", "fr"),
  l(SubTotal, "Subtotal", "es"),
  l(SubTotal, "Base imponible", "es"),
  l(SubTotal, "Importe neto", "es"),
  l(SubTotal, "Total neto", "es"),
  l(Tax, "Tax", "en"),
  l(Tax, "Sales Tax", "en"),
  l(Tax, "VAT", "en"),
  l(Tax, "GST", "en"),
  l(Tax, "HST", "en"),
  l(Tax, "PST", "en"),
  l(Tax, "QST", "en"),
  l(Tax, "TPS", "fr"),
  l(Tax, "TVQ", "fr"),
  l(Tax, "TVA", "fr"),
  l(Tax, "MwSt", "de"),
  l(Tax, "MwSt.", "de"),
  l(Tax, "USt", "de"),
  l(Tax, "Umsatzsteuer", "de"),
  l(Tax, "IVA", "es"),
  l(Tax, "Impuesto", "es"),
  l(Tax, "Impuestos", "es"),
  l(Tax, "Taxe", "fr"),
  l(Tax, "Taxes", "fr"),
  l(Tax, "Steuer", "de"),
  l(Total, "Total", "en"),
  l(Total, "Total", "es"),
  l(Total, "Total", "fr"),
  l(Total, "Total", "pt"),
  l(Total, "Invoice Total", "en"),
  l(Total, "Total Amount", "en"),
  l(Total, "Grand Total", "en"),
  l(Total, "Total Payable", "en"),
  l(Total, "Invoice Amount", "en"),
  l(Total, "Total Invoice", "en"),
  l(Total, "Total Charges", "en"),
  l(Total, "Total (USD)", "en"),
  l(Total, "Total (CAD)", "en"),
  l(Total, "Total (EUR)", "en"),
  l(Total, "Total (GBP)", "en"),
  l(Total, "Total USD", "en"),
  l(Total, "USD Total", "en"),
  l(Total, "Total in USD", "en"),
  l(Total, "Total Credit", "en"),
  l(Total, "Rechnungsbetrag", "de"),
  l(Total, "Gesamtbetrag", "de"),
  l(Total, "Gesamt", "de"),
  l(Total, "Brutto", "de"),
  l(Total, "Montant total", "fr"),
  l(Total, "Total TTC", "fr"),
  l(Total, "Montant", "fr"),
  l(Total, "Importe total", "es"),
  l(Total, "Importe", "es"),
  l(Total, "Totale", "it"),
  l(Total, "Valor total", "es"),
  l(Total, "Total factura", "es"),
  l(Total, "Total presupuesto", "es"),
  l(Total, "Total general", "es"),
  l(Total, "Monto total", "es"),
  l(Total, "Total devis", "fr"),
  l(Total, "Endbetrag", "de"),
  l(AmountDue, "Amount Due", "en"),
  l(AmountDue, "Balance Due", "en"),
  l(AmountDue, "Balance", "en"),
  l(AmountDue, "Total Amount Due", "en"),
  l(AmountDue, "Total Due", "en"),
  l(AmountDue, "Please Pay", "en"),
  l(AmountDue, "Zu zahlen", "de"),
  l(AmountDue, "Zahlbetrag", "de"),
  l(AmountDue, "Montant dû", "fr"),
  l(AmountDue, "Total a pagar", "es"),
  l(AmountDue, "Importe a pagar", "es"),
  l(AmountDue, "Monto a pagar", "es"),
  l(AmountDue, "Saldo", "es"),
  l(AmountDue, "Saldo pendiente", "es"),
  l(AmountDue, "Reste à payer", "fr"),
  l(AmountDue, "Offener Betrag", "de"),
  l(AmountDue, "Net à payer", "fr"),
  // stop words
  l(Terms, "Terms", "en"),
  l(Terms, "Payment Terms", "en"),
  l(Terms, "Net 30", "en"),
  l(Terms, "Net 60", "en"),
  l(Terms, "Zahlungsbedingungen", "de"),
  l(Terms, "Zahlungsziel", "de"),
  l(Terms, "Condiciones de pago", "es"),
  l(Terms, "Forma de pago", "es"),
  l(Terms, "Términos de pago", "es"),
  l(Terms, "Conditions de paiement", "fr"),
  l(Terms, "Modalités de paiement", "fr"),
  // work orders
  l(WorkOrder, "Work Order", "en"),
  l(WorkOrder, "Work Order No", "en"),
  l(WorkOrder, "Work Order Number", "en"),
  l(WorkOrder, "Work Order #", "en"),
  l(WorkOrder, "WO", "en"),
  l(WorkOrder, "WO #", "en"),
  l(WorkOrder, "WO No.", "en"),
  l(WorkOrder, "W/O No.", "en"),
  l(WorkOrder, "Workorder", "en"),
  l(WorkOrder, "Sales Order", "en"),
  l(WorkOrder, "Sales Order No.", "en"),
  l(WorkOrder, "Sales Order Number", "en"),
  l(WorkOrder, "Sales Order #", "en"),
  l(WorkOrder, "SO No.", "en"),
  l(WorkOrder, "SO #", "en"),
  l(WorkOrder, "Shop Order", "en"),
  l(WorkOrder, "Shop Order No.", "en"),
  l(WorkOrder, "Job Number", "en"),
  l(WorkOrder, "Job Card", "en"),
  l(WorkOrder, "Job No.", "en"),
  l(WorkOrder, "Maintenance Order No.", "en"),
  l(WorkOrder, "Cust RO", "en"),
  l(WorkOrder, "Cust RO #", "en"),
  l(WorkOrder, "Auftragsnummer", "de"),
  l(WorkOrder, "Orden de trabajo", "es"),
  l(WorkOrder, "Orden de trabajo No.", "es"),
  l(WorkOrder, "O.T.", "es"),
  l(WorkOrder, "Ordre de travail", "fr"),
  l(WorkOrder, "Bon de travail", "fr"),
  // aircraft
  l(Registration, "Tail", "en"),
  l(Registration, "Tail No.", "en"),
  l(Registration, "Tail Number", "en"),
  l(Registration, "Tail #", "en"),
  l(Registration, "Registration", "en"),
  l(Registration, "Registration Number", "en"),
  l(Registration, "Aircraft Registration", "en"),
  l(Registration, "Reg", "en"),
  l(Registration, "Reg.", "en"),
  l(Registration, "A/C Reg.", "en"),
  l(Registration, "Aircraft Reg", "en"),
  l(Registration, "Aircraft Number", "en"),
  l(Registration, "Aircraft No.", "en"),
  l(Registration, "A/C Number", "en"),
  l(Registration, "A/C No.", "en"),
  l(Registration, "A/C Reg", "en"),
  l(Registration, "Aircraft Reg.", "en"),
  l(Registration, "Immatriculation", "fr"),
  l(Registration, "Matrícula", "es"),
  l(Registration, "Siglas", "es"),
  l(Registration, "Kennzeichen", "de"),
  l(Registration, "Luftfahrzeugkennzeichen", "de"),
  l(Serial, "Serial No.", "en"),
  l(Serial, "Serial Number", "en"),
  l(Serial, "Serial #", "en"),
  l(Serial, "S/N", "en"),
  l(Serial, "SN", "en"),
  l(Serial, "SN#", "en"),
  l(Serial, "A/C S/N", "en"),
  l(Serial, "AC Serial No.", "en"),
  l(Serial, "Aircraft Serial Number", "en"),
  l(Serial, "Aircraft SN", "en"),
  l(Serial, "MSN", "en"),
  l(Serial, "Airframe", "en"),
  l(Serial, "Seriennummer", "de"),
  l(Serial, "Numéro de série", "fr"),
  l(Serial, "Serial", "es"),
  l(Serial, "Serial No", "es"),
  l(Serial, "Número de serie", "es"),
  l(Serial, "No. de serie", "es"),
  l(EngineSerial, "ESN", "en"),
  l(EngineSerial, "Engine Serial", "en"),
  l(EngineSerial, "Engine S/N", "en"),
  l(EngineSerial, "Engine SN", "en"),
  l(EngineSerial, "Engine Serial #", "en"),
  l(EngineSerial, "Engine Serial Number", "en"),
  l(EngineSerial, "Serial motor", "es"),
  l(EngineSerial, "Serial del motor", "es"),
  l(EngineSerial, "No. de serie del motor", "es"),
  l(EngineSerial, "Numéro de série moteur", "fr"),
  l(EngineSerial, "Triebwerksseriennummer", "de"),
  // credit titles
  l(CreditTitle, "Credit Memo", "en"),
  l(CreditTitle, "Credit Note", "en"),
  l(CreditTitle, "Credit Memorandum", "en"),
  l(CreditTitle, "Debit Memo", "en"),
  l(CreditTitle, "Debit Note", "en"),
  l(CreditTitle, "Note de crédit", "fr"),
  l(CreditTitle, "Avoir", "fr"),
  l(CreditTitle, "Gutschrift", "de"),
  l(CreditTitle, "Nota de crédito", "es"),
  // document titles: what a vendor line is not
  l(Other, "Ship Date", "en"),
  l(Other, "Shipping Date", "en"),
  l(Other, "Shipped Date", "en"),
  l(Other, "Date Shipped", "en"),
  l(Other, "Order Date", "en"),
  l(Other, "Date Ordered", "en"),
  l(Other, "Delivery Date", "en"),
  l(Other, "Date of Delivery", "en"),
  l(Other, "Service Date", "en"),
  l(Other, "Date of Service", "en"),
  l(Other, "Start Date", "en"),
  l(Other, "End Date", "en"),
  l(Other, "Expiry Date", "en"),
  l(Other, "Expiration Date", "en"),
  l(Other, "Date Received", "en"),
  l(Other, "Received Date", "en"),
  l(Other, "Billing Period", "en"),
  l(Other, "Service Period", "en"),
  l(Other, "Previous Balance", "en"),
  l(Other, "Balance Forward", "en"),
  l(Other, "Payments Received", "en"),
  l(Other, "Payment Received", "en"),
  l(Other, "Less Payments", "en"),
  l(Other, "Customer Project Manager", "en"),
  l(Other, "Customer Service", "en"),
  l(Other, "Customer Service Rep", "en"),
  l(Other, "Customer Service Representative", "en"),
  l(Other, "Customer Contact", "en"),
  l(Other, "Customer Representative", "en"),
  l(Other, "Customer Rep", "en"),
  l(Other, "Customer Care", "en"),
  l(Other, "Customer Support", "en"),
  l(Other, "Customer Signature", "en"),
  l(Other, "Lieferdatum", "de"),
  l(Other, "Leistungsdatum", "de"),
  l(Other, "Bestelldatum", "de"),
  l(Other, "Liefertermin", "de"),
  l(Other, "Date de livraison", "fr"),
  l(Other, "Date de commande", "fr"),
  l(Other, "Date d'expédition", "fr"),
  l(Other, "Fecha de entrega", "es"),
  l(Other, "Fecha de pedido", "es"),
  l(Other, "Fecha de envío", "es"),
  l(Other, "Aeronave tipo", "es"),
  l(Other, "Tipo de aeronave", "es"),
  l(Other, "Motor tipo", "es"),
  l(Other, "Tipo de motor", "es"),
  l(Other, "Horas totales", "es"),
  l(Other, "Horas", "es"),
  l(Other, "Type d'aéronef", "fr"),
  l(Other, "Luftfahrzeugtyp", "de"),
  l(DocumentTitle, "Invoice", "en"),
  l(DocumentTitle, "Tax Invoice", "en"),
  l(DocumentTitle, "Commercial Invoice", "en"),
  l(DocumentTitle, "Pro-Forma Invoice", "en"),
  l(DocumentTitle, "Proforma", "en"),
  l(DocumentTitle, "Form", "en"),
  l(DocumentTitle, "Statement", "en"),
  l(DocumentTitle, "Receipt", "en"),
  l(DocumentTitle, "Quote", "en"),
  l(DocumentTitle, "Quotation", "en"),
  l(DocumentTitle, "Estimate", "en"),
  l(DocumentTitle, "Purchase Order", "en"),
  l(DocumentTitle, "Packing Slip", "en"),
  l(DocumentTitle, "Remittance Advice", "en"),
  l(DocumentTitle, "Credit Memo", "en"),
  l(DocumentTitle, "Credit Note", "en"),
  l(DocumentTitle, "Rechnung", "de"),
  l(DocumentTitle, "Gutschrift", "de"),
  l(DocumentTitle, "Facture", "fr"),
  l(DocumentTitle, "Factura", "es"),
  l(DocumentTitle, "Presupuesto", "es"),
  l(DocumentTitle, "Cotización", "es"),
  l(DocumentTitle, "Recibo", "es"),
  l(DocumentTitle, "Orden de compra", "es"),
  l(DocumentTitle, "Albarán", "es"),
  l(DocumentTitle, "Remito", "es"),
  l(DocumentTitle, "Devis", "fr"),
  l(DocumentTitle, "Angebot", "de"),
  l(DocumentTitle, "Lieferschein", "de"),
];

/// Lower-case, OCR confusables folded, punctuation and whitespace dropped. `#` survives, so
/// `Invoice #` and `Invoice` stay different labels.
pub(crate) fn normalize(text: &str) -> String {
  let mut out = String::with_capacity(text.len());
  for c in text.chars() {
    // Case first, confusables second: `L` and `l` must fold alike (`TOTAL` is `Total`).
    let lower = if c.is_alphanumeric() {
      c.to_lowercase().next().unwrap_or(c)
    } else {
      c
    };
    let folded = match lower {
      '0' | 'ο' | 'о' => 'o',
      '1' | 'l' | '|' | 'i' | 'í' | 'ì' | 'ı' => 'i',
      '5' => 's',
      '#' => '#',
      c if c.is_alphanumeric() => c,
      _ => continue,
    };
    out.push(folded);
  }
  out
}

/// A label found in a segment: the row, how well it matched, the words it occupies
/// (`start..end`, `end` exclusive) and the text that follows it in the segment.
#[derive(Debug, Clone)]
pub(crate) struct Hit {
  pub label: &'static Label,
  pub score: f64,
  pub start: usize,
  pub end: usize,
  pub remainder: String,
}

/// Characters a value may be glued to a label with (`Invoice #13594027`, `No.2015-0831`).
const GLUE: &[char] = &['#', ':', '.', '-', '/'];

/// The labels with their normalised forms, computed once.
struct NormLabel {
  label: &'static Label,
  norm: String,
  chars: usize,
  charset: u64,
}

/// The characters of a text as one bit per character class: every edit adds or removes at most
/// two members, so a symmetric difference above `2 · max` rules a fuzzy match out before the
/// edit distance is computed.
fn charset(text: &str) -> u64 {
  text
    .chars()
    .fold(0u64, |set, c| set | 1u64 << (c as u32 % 64))
}

/// The longest normalised label, in bytes.
fn longest_label() -> usize {
  static CACHE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
  *CACHE.get_or_init(|| {
    norm_labels()
      .iter()
      .map(|nl| nl.norm.len())
      .max()
      .unwrap_or(0)
  })
}

fn norm_labels() -> &'static [NormLabel] {
  static CACHE: std::sync::OnceLock<Vec<NormLabel>> = std::sync::OnceLock::new();
  CACHE.get_or_init(|| {
    LABELS
      .iter()
      .map(|label| {
        let norm = normalize(label.text);
        NormLabel {
          label,
          chars: norm.chars().count(),
          charset: charset(&norm),
          norm,
        }
      })
      .collect()
  })
}

/// One attempt to match a normalised label at word `start`, given the words' normalised forms:
/// the label must begin the normalised text of the words from `start`; a match may end inside a
/// word only at a glue character or a digit. Returns the exclusive word index after the label
/// and the remainder text.
fn match_at(
  words: &[&str],
  norms: &[String],
  start: usize,
  label_norm: &str,
) -> Option<(usize, String)> {
  let mut built_len = 0usize;
  let mut built = String::new();
  for i in start..words.len() {
    let before = built_len;
    let norm_word = &norms[i];
    built_len += norm_word.len();
    if built_len < label_norm.len() {
      built.push_str(norm_word);
      if !label_norm.starts_with(built.as_str()) {
        return None;
      }
      continue;
    }
    built.push_str(norm_word);
    if !built.starts_with(label_norm) {
      return None;
    }
    let consumed_in_word = label_norm.len() - before;
    if consumed_in_word == norm_word.len() {
      let rest = words[i + 1..].join(" ");
      return Some((i + 1, trim_glue(&rest)));
    }
    // Ends inside the word: find the raw split after `consumed_in_word` normalised bytes.
    let word = words[i];
    let mut seen = 0usize;
    let mut split: Option<usize> = None;
    for (idx, c) in word.char_indices() {
      if seen >= consumed_in_word {
        split = Some(idx);
        break;
      }
      seen += normalize(&c.to_string()).len();
    }
    let split = split?;
    let after = &word[split..];
    let next = after.chars().next()?;
    if !(GLUE.contains(&next) || next.is_ascii_digit()) {
      return None;
    }
    let mut rest = after.to_string();
    if i + 1 < words.len() {
      rest.push(' ');
      rest.push_str(&words[i + 1..].join(" "));
    }
    return Some((i + 1, trim_glue(&rest)));
  }
  None
}

fn trim_glue(text: &str) -> String {
  text
    .trim()
    .trim_start_matches(|c: char| GLUE.contains(&c) || c.is_whitespace())
    .trim()
    .to_string()
}

/// Bounded Levenshtein distance; `None` when it exceeds `max`.
fn edit_distance(a: &str, b: &str, max: usize) -> Option<usize> {
  if a.chars().count().abs_diff(b.chars().count()) > max {
    return None;
  }
  let a: Vec<char> = a.chars().collect();
  let b: Vec<char> = b.chars().collect();
  let mut prev: Vec<usize> = (0..=b.len()).collect();
  let mut cur = vec![0; b.len() + 1];
  for (i, ca) in a.iter().enumerate() {
    cur[0] = i + 1;
    let mut row_min = cur[0];
    for (j, cb) in b.iter().enumerate() {
      let cost = usize::from(ca != cb);
      cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
      row_min = row_min.min(cur[j + 1]);
    }
    if row_min > max {
      return None;
    }
    std::mem::swap(&mut prev, &mut cur);
  }
  (prev[b.len()] <= max).then_some(prev[b.len()])
}

/// Every label of `field` (any field when `None`) found in the segment's words, best first.
/// A label matching the longest stretch at a position scores 1.0; a shorter label that is a
/// prefix of a longer match there scores 0.8; a fuzzy match (one edit per five characters, whole
/// words only) scores 0.6. A label starting inside a longer label of another field is not a hit;
/// a label that does not open its segment scores 0.8 of its match; a terms line labels nothing.
pub(crate) fn find_labels(words: &[&str], field: Option<Field>) -> Vec<Hit> {
  let norms: Vec<String> = words.iter().map(|w| normalize(w)).collect();
  let labels = norm_labels();
  let mut hits: Vec<Hit> = Vec::new();
  for start in 0..words.len() {
    if norms[start].is_empty() {
      continue;
    }
    let first = norms[start].chars().next();
    // The longest label of any field matching here decides who is a prefix.
    let mut longest = 0usize;
    let mut exact: Vec<(&NormLabel, usize, String)> = Vec::new();
    for nl in labels {
      if nl.norm.chars().next() != first {
        continue;
      }
      if let Some((end, remainder)) = match_at(words, &norms, start, &nl.norm) {
        longest = longest.max(nl.norm.len());
        exact.push((nl, end, remainder));
      }
    }
    let any_exact = !exact.is_empty();
    for (nl, end, remainder) in exact {
      let score = if nl.norm.len() == longest { 1.0 } else { 0.8 };
      hits.push(Hit {
        label: nl.label,
        score,
        start,
        end,
        remainder,
      });
    }
    // Fuzzy: whole-word prefixes only, and only where nothing exact matched. The words from
    // `start` are joined once, as far as the longest label could reach, and a label is compared
    // only against the joins whose length and character set allow a match.
    if !any_exact && norms[start].len() >= 4 {
      let reach = longest_label() + longest_label() / 5;
      let mut built = String::new();
      let mut steps: Vec<(usize, String, usize, u64)> = Vec::new();
      for (i, norm) in norms.iter().enumerate().skip(start) {
        built.push_str(norm);
        if built.len() > reach {
          break;
        }
        steps.push((i + 1, built.clone(), built.chars().count(), charset(&built)));
      }
      for nl in labels {
        let max = nl.norm.len() / 5;
        if max == 0 {
          continue;
        }
        for (end, text, chars, set) in &steps {
          if *chars + max < nl.chars {
            continue;
          }
          if *chars > nl.chars + max {
            break;
          }
          if (set ^ nl.charset).count_ones() as usize > 2 * max {
            continue;
          }
          if edit_distance(text, &nl.norm, max).is_some() {
            hits.push(Hit {
              label: nl.label,
              score: 0.6,
              start,
              end: *end,
              remainder: trim_glue(&words[*end..].join(" ")),
            });
            break;
          }
        }
      }
    }
  }
  // A terms line (`Terms: Net 30`) labels nothing but itself.
  if starts_with_terms(&norms) {
    hits.retain(|h| h.label.field == Terms);
  }
  // A label that starts strictly inside another field's exact match (`Date` in `Due Date`,
  // `Total` in `Sub Total`, `Order No.` in `Sales Order`) is not a hit, nor is one that merely
  // opens a label of nothing this reader extracts (`Tax` in `Tax Invoice`, `Date` in `Date
  // Shipped`); a label that does not open its segment scores 0.8 of its match.
  let spans: Vec<(Field, usize, usize, f64)> = hits
    .iter()
    .map(|h| (h.label.field, h.start, h.end, h.score))
    .collect();
  hits.retain(|h| {
    !spans.iter().any(|&(field, start, end, score)| {
      field != h.label.field
        && score >= 1.0
        && h.start < end
        && (start < h.start
          || (start == h.start && end > h.end && matches!(field, DocumentTitle | Other)))
    })
  });
  for h in hits.iter_mut() {
    if h.start > 0 {
      h.score *= 0.8;
    }
  }
  hits.retain(|h| h.score >= 0.6);
  if let Some(f) = field {
    hits.retain(|h| h.label.field == f);
  }
  hits.sort_by(|a, b| {
    b.score
      .partial_cmp(&a.score)
      .unwrap_or(std::cmp::Ordering::Equal)
      .then(a.start.cmp(&b.start))
      .then(b.end.cmp(&a.end))
  });
  // Rows that normalise alike (`MwSt`, `MwSt.`) are one hit.
  hits.dedup_by(|a, b| {
    a.label.field == b.label.field && a.start == b.start && a.end == b.end && a.score == b.score
  });
  hits
}

/// Whether the segment opens with a terms stop word, checked against the full table.
fn starts_with_terms(norms: &[String]) -> bool {
  norm_labels()
    .iter()
    .filter(|nl| nl.label.field == Terms)
    .any(|nl| {
      let mut built = String::new();
      for n in norms {
        built.push_str(n);
        if built.len() >= nl.norm.len() {
          return built == nl.norm;
        }
      }
      false
    })
}

/// Whether a label row is also a document title (`Invoice`, `Tax Invoice`, `Credit Memo`): such a
/// row anchors a value only on its own line (`Invoice #13594027`), never below or beside it.
pub(crate) fn is_title_row(label: &Label) -> bool {
  static TITLES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
  let titles = TITLES.get_or_init(|| {
    LABELS
      .iter()
      .filter(|l| l.field == DocumentTitle)
      .map(|l| normalize(l.text))
      .collect()
  });
  label.field != DocumentTitle && titles.contains(&normalize(label.text))
}

/// Whether a segment opens with a terms stop word, from its unfiltered hits.
pub(crate) fn has_terms_at_start(hits: &[Hit]) -> bool {
  hits
    .iter()
    .any(|h| h.start == 0 && h.label.field == Terms && h.score >= 0.8)
}

/// Whether a label's text belongs to one language only: `Total`, `Date` and `Serial No` are rows
/// of several languages and say nothing about the document's language; `Fecha` or `Rechnung` do.
pub(crate) fn language_specific(label: &Label) -> bool {
  static SHARED: std::sync::OnceLock<std::collections::HashSet<String>> =
    std::sync::OnceLock::new();
  let shared = SHARED.get_or_init(|| {
    let mut langs: std::collections::HashMap<String, std::collections::HashSet<&str>> =
      std::collections::HashMap::new();
    for l in LABELS {
      langs.entry(normalize(l.text)).or_default().insert(l.lang);
    }
    langs
      .into_iter()
      .filter(|(_, ls)| ls.len() > 1)
      .map(|(text, _)| text)
      .collect()
  });
  !shared.contains(&normalize(label.text))
}

/// What a page declares itself to be in its top band. Our own purchase order, a packing slip or
/// delivery note, and a remittance advice travel with invoices but are not the invoice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PageRole {
  Invoice,
  PurchaseOrder,
  Delivery,
  Remittance,
}

const ROLE_TITLES: &[(PageRole, &str)] = &[
  (PageRole::PurchaseOrder, "Purchase Order"),
  (PageRole::PurchaseOrder, "Orden de compra"),
  (PageRole::PurchaseOrder, "Bon de commande"),
  (PageRole::PurchaseOrder, "Bestellung"),
  (PageRole::Delivery, "Packing Slip"),
  (PageRole::Delivery, "Packing List"),
  (PageRole::Delivery, "Delivery Note"),
  (PageRole::Delivery, "Lieferschein"),
  (PageRole::Delivery, "Albarán"),
  (PageRole::Delivery, "Remito"),
  (PageRole::Delivery, "Bon de livraison"),
  (PageRole::Remittance, "Remittance Advice"),
  (PageRole::Remittance, "Remittance"),
];

/// The role a title segment declares: a role title alone on its segment (`PURCHASE ORDER`, not
/// `Purchase Order: 249604`, which is a label with a value), any other document title is the
/// invoice's own, anything else is no title.
pub(crate) fn role_of_title(words: &[&str]) -> Option<PageRole> {
  let whole = normalize(&words.join(" "));
  if whole.is_empty() {
    return None;
  }
  if let Some((role, _)) = ROLE_TITLES.iter().find(|(_, t)| normalize(t) == whole) {
    return Some(*role);
  }
  LABELS
    .iter()
    .any(|l| l.field == DocumentTitle && normalize(l.text) == whole)
    .then_some(PageRole::Invoice)
}

/// Whether a segment is the document's title rather than a name: every word is a title word, or
/// the whole segment is a title row, or the segment ends with a title word (`… Form`).
pub(crate) fn is_document_title(words: &[&str]) -> bool {
  let titles: Vec<String> = LABELS
    .iter()
    .filter(|l| l.field == DocumentTitle)
    .map(|l| normalize(l.text))
    .collect();
  let whole = normalize(&words.join(" "));
  if titles.contains(&whole) {
    return true;
  }
  let word_norms: Vec<String> = words
    .iter()
    .map(|w| normalize(w))
    .filter(|w| !w.is_empty())
    .collect();
  if word_norms.is_empty() {
    return false;
  }
  let is_title_word = |w: &String| titles.iter().any(|t| t == w);
  word_norms.iter().all(is_title_word) || word_norms.last().is_some_and(is_title_word)
}

#[cfg(test)]
mod tests;
