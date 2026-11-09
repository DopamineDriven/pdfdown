use napi::bindgen_prelude::Buffer;
use napi_derive::napi;

// ── Attachments ───────────────────────────────────────────────────

/// A file embedded in the PDF: a `/Names /EmbeddedFiles` entry, a catalog
/// `/AF` associated file, or a page-level `/FileAttachment` annotation.
#[napi(object)]
pub struct Attachment {
  pub name: String,
  /// From the embedded file stream's `/Subtype`, e.g. `text/xml`.
  pub mime_type: Option<String>,
  /// Decoded payload size in bytes.
  pub size: u32,
  /// `/AFRelationship`: `Source`, `Data`, `Alternative`, `Supplement`, ...
  pub relationship: Option<String>,
  pub description: Option<String>,
  /// 1-based page for a `/FileAttachment` annotation; absent for document-level files.
  pub page: Option<u32>,
  pub data: Buffer,
}

/// Thread-safe twin of `Attachment` (no napi handles), used inside `AsyncTask`.
#[derive(Debug, Clone)]
pub struct RawAttachment {
  pub name: String,
  pub mime_type: Option<String>,
  pub relationship: Option<String>,
  pub description: Option<String>,
  pub page: Option<u32>,
  pub data: Vec<u8>,
}

impl From<RawAttachment> for Attachment {
  fn from(r: RawAttachment) -> Self {
    Attachment {
      name: r.name,
      mime_type: r.mime_type,
      size: r.data.len() as u32,
      relationship: r.relationship,
      description: r.description,
      page: r.page,
      data: r.data.into(),
    }
  }
}

// ── Invoice result shape (shared by every extraction source) ──────

/// Where a field was read from, in PDF user space on a 1-based page.
#[derive(Debug, Clone, PartialEq)]
#[napi(object)]
pub struct FieldSpan {
  pub page: u32,
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
}

#[derive(Debug, Clone, PartialEq)]
#[napi(object)]
pub struct StringField {
  pub value: String,
  /// The exact source text the value was derived from.
  pub text: String,
  pub confidence: f64,
  pub span: Option<FieldSpan>,
}

#[derive(Debug, Clone, PartialEq)]
#[napi(object)]
pub struct NumberField {
  pub value: f64,
  pub text: String,
  pub confidence: f64,
  pub span: Option<FieldSpan>,
}

/// `value` is an ISO 8601 calendar date, `YYYY-MM-DD`.
#[derive(Debug, Clone, PartialEq)]
#[napi(object)]
pub struct DateField {
  pub value: String,
  pub text: String,
  pub confidence: f64,
  pub span: Option<FieldSpan>,
}

/// Money as a JS number plus the exact printed text; `text` is authoritative
/// when cents matter.
#[derive(Debug, Clone, PartialEq)]
#[napi(object)]
pub struct MoneyField {
  pub value: f64,
  pub text: String,
  pub confidence: f64,
  pub span: Option<FieldSpan>,
  pub currency_code: Option<String>,
  pub currency_symbol: Option<String>,
}

/// `value` is `lines` joined with `\n`.
#[derive(Debug, Clone, PartialEq)]
#[napi(object)]
pub struct AddressField {
  pub value: String,
  pub text: String,
  pub confidence: f64,
  pub span: Option<FieldSpan>,
  pub lines: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
#[napi(object)]
pub struct InvoiceItem {
  pub description: Option<StringField>,
  pub product_code: Option<StringField>,
  pub quantity: Option<NumberField>,
  pub unit: Option<StringField>,
  pub unit_price: Option<MoneyField>,
  pub tax_rate: Option<NumberField>,
  pub tax: Option<MoneyField>,
  pub discount: Option<MoneyField>,
  pub date: Option<DateField>,
  pub amount: Option<MoneyField>,
  /// Absent when the item came from embedded XML (no page geometry).
  pub span: Option<FieldSpan>,
}

#[derive(Debug, Clone, Default, PartialEq)]
#[napi(object)]
pub struct TaxDetail {
  pub rate: Option<NumberField>,
  pub amount: Option<MoneyField>,
  pub base: Option<MoneyField>,
  pub category: Option<StringField>,
}

#[derive(Debug, Clone, Default, PartialEq)]
#[napi(object)]
pub struct PaymentDetail {
  pub iban: Option<StringField>,
  pub swift: Option<StringField>,
  pub bank_name: Option<StringField>,
  pub account_number: Option<StringField>,
  pub routing_number: Option<StringField>,
}

/// An aircraft or engine the invoice relates to. Maintenance, parts and
/// program invoices identify the asset by airframe (model, manufacturer serial
/// number, registration) and/or by engine (model, engine serial number); one
/// invoice can cover several assets, so `InvoiceFields.aircraft` is a list.
#[derive(Debug, Clone, Default, PartialEq)]
#[napi(object)]
pub struct AircraftReference {
  /// Airframe model, e.g. "Challenger 350", "Global 6000", "Phenom 300".
  pub model: Option<StringField>,
  /// Manufacturer serial number (MSN).
  pub serial_number: Option<StringField>,
  /// Registration / tail number, e.g. "N123AB", "C-GXYZ".
  pub registration: Option<StringField>,
  /// Engine model, e.g. "BR710", "AE 3007", "PW308C".
  pub engine_model: Option<StringField>,
  /// Engine serial number (ESN).
  pub engine_serial_number: Option<StringField>,
  pub span: Option<FieldSpan>,
}

/// Field names mirror the de-facto vocabulary of common invoice extractors so
/// consumers can swap sources with a thin mapper. Naming only: nothing here
/// talks to any service.
#[derive(Debug, Clone, Default, PartialEq)]
#[napi(object)]
pub struct InvoiceFields {
  pub vendor_name: Option<StringField>,
  pub vendor_address: Option<AddressField>,
  pub vendor_tax_id: Option<StringField>,
  pub vendor_email: Option<StringField>,
  pub vendor_phone: Option<StringField>,
  pub vendor_website: Option<StringField>,
  pub customer_name: Option<StringField>,
  pub customer_id: Option<StringField>,
  pub customer_address: Option<AddressField>,
  pub customer_tax_id: Option<StringField>,
  pub billing_address: Option<AddressField>,
  pub shipping_address: Option<AddressField>,
  pub remittance_address: Option<AddressField>,
  pub invoice_id: Option<StringField>,
  pub invoice_date: Option<DateField>,
  pub due_date: Option<DateField>,
  pub purchase_order: Option<StringField>,
  pub payment_term: Option<StringField>,
  pub service_start_date: Option<DateField>,
  pub service_end_date: Option<DateField>,
  pub currency_code: Option<String>,
  pub sub_total: Option<MoneyField>,
  pub total_discount: Option<MoneyField>,
  pub total_tax: Option<MoneyField>,
  pub shipping_cost: Option<MoneyField>,
  pub invoice_total: Option<MoneyField>,
  pub amount_due: Option<MoneyField>,
  pub previous_unpaid_balance: Option<MoneyField>,
  /// MRO work order / shop order the invoice bills against.
  pub work_order: Option<StringField>,
  /// Aircraft and engines the invoice relates to; empty when none is identified.
  pub aircraft: Vec<AircraftReference>,
  pub tax_details: Vec<TaxDetail>,
  pub payment_details: Vec<PaymentDetail>,
  pub items: Vec<InvoiceItem>,
}

/// Which extraction path produced the result: embedded XML, the page text (native, OCR,
/// rendered or mixed), a workbook's cells, or `None` when the document had no readable
/// pages and no embedded XML.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum)]
pub enum InvoiceSource {
  EmbeddedXml,
  Native,
  Ocr,
  Rendered,
  Mixed,
  /// The logical pages of a workbook.
  Spreadsheet,
  None,
}

/// Which tier named the currency: evidence on the page (an ISO code or an unambiguous glyph),
/// content priors such as the vendor's country, the caller's option, or nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum)]
pub enum CurrencySource {
  Page,
  Content,
  Option,
  Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum)]
pub enum EInvoiceStandard {
  FacturX,
  ZUGFeRD,
  XRechnung,
  UBL,
  Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum)]
pub enum InvoiceDocumentType {
  Invoice,
  CreditNote,
  Unknown,
}

#[derive(Debug, Clone, PartialEq)]
#[napi(object)]
pub struct EmbeddedInvoiceXml {
  pub filename: String,
  pub standard: EInvoiceStandard,
  /// The guideline / customization identifier, e.g.
  /// `urn:cen.eu:en16931:2017#compliant#urn:factur-x.eu:1p0:basic`.
  pub profile: Option<String>,
  pub xml: String,
}

#[derive(Debug, Clone, PartialEq)]
#[napi(object, object_from_js = false)]
pub struct InvoiceResult {
  pub source: InvoiceSource,
  pub document_type: InvoiceDocumentType,
  /// Document-level confidence, 0..1.
  pub confidence: f64,
  pub fields: InvoiceFields,
  pub warnings: Vec<String>,
  pub embedded_xml: Option<EmbeddedInvoiceXml>,
  /// Which tier named `fields.currencyCode`; absent when no layout pass ran.
  pub currency_source: Option<CurrencySource>,
  /// The evidence behind `documentType: CreditNote`; empty otherwise.
  pub credit_signals: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum)]
pub enum DateOrder {
  Auto,
  MDY,
  DMY,
  YMD,
}

/// Options accepted by every invoice entry point. `preferEmbeddedXml` picks the embedded
/// XML over the pages when both exist; `languages`, `currency`, `dateOrder` and
/// `decimalSeparator` steer the page reader where the page leaves a choice;
/// `includeCandidates` lists, in `warnings`, the candidates each field was chosen from;
/// `includeLayout` is reserved.
#[derive(Debug, Clone, Default)]
#[napi(object)]
pub struct InvoiceOptions {
  /// Label lexicons to enable; default `["en"]`.
  pub languages: Option<Vec<String>>,
  /// ISO 4217 hint when the page shows only symbols.
  pub currency: Option<String>,
  pub date_order: Option<DateOrder>,
  #[napi(ts_type = "\"Auto\" | \".\" | \",\"")]
  pub decimal_separator: Option<String>,
  /// Default `true`. Embedded Factur-X / ZUGFeRD / XRechnung / UBL XML is used when present.
  pub prefer_embedded_xml: Option<bool>,
  /// Reserved; accepted and ignored.
  pub include_layout: Option<bool>,
  /// Default `false`. Lists, in `warnings`, the candidates each field was chosen from, with
  /// their scores.
  pub include_candidates: Option<bool>,
}
