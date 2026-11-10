//! Embedded e-invoice XML: detection among attachments and mapping of
//! UN/CEFACT CII (Factur-X, ZUGFeRD, XRechnung CII) and OASIS UBL 2.x
//! (Invoice, CreditNote, XRechnung UBL) onto `InvoiceFields`.
//!
//! Element lookup is by local name, so namespace prefixes (`rsm:`, `ram:`,
//! `cac:`, `cbc:`, or none) never matter. Every mapped field carries
//! confidence 1.0 and no span: the XML is the authoritative source.

use crate::types::{
  AddressField, DateField, EInvoiceStandard, InvoiceDocumentType, InvoiceFields, InvoiceItem,
  MoneyField, NumberField, PaymentDetail, RawAttachment, StringField, TaxDetail,
};
use roxmltree::Node;

const UBL_NAMESPACE_MARKER: &str = "oasis:names:specification:ubl";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum XmlSyntax {
  Cii,
  Ubl,
}

pub(crate) struct DetectedXml {
  pub attachment_index: usize,
  pub syntax: XmlSyntax,
  pub standard: EInvoiceStandard,
  pub profile: Option<String>,
  pub xml: String,
}

const KNOWN_FILENAMES: &[&str] = &[
  "factur-x.xml",
  "zugferd-invoice.xml",
  "xrechnung.xml",
  "order-x.xml",
  "ubl-invoice.xml",
];

fn is_xml_candidate(a: &RawAttachment) -> bool {
  let name = a.name.to_ascii_lowercase();
  let rel = a.relationship.as_deref().unwrap_or("");
  matches!(rel, "Data" | "Alternative" | "Source")
    || KNOWN_FILENAMES.contains(&name.as_str())
    || name.ends_with(".xml")
    || a.mime_type.as_deref().is_some_and(|m| {
      m.eq_ignore_ascii_case("text/xml") || m.eq_ignore_ascii_case("application/xml")
    })
}

/// Decode attachment bytes as text, tolerating a UTF-8 BOM.
fn xml_text(data: &[u8]) -> Option<String> {
  let data = data.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(data);
  std::str::from_utf8(data).ok().map(str::to_string)
}

fn standard_from_identifier(id: &str, filename: &str, syntax: XmlSyntax) -> EInvoiceStandard {
  let id = id.to_ascii_lowercase();
  let name = filename.to_ascii_lowercase();
  if id.contains("xrechnung") || name.contains("xrechnung") {
    return EInvoiceStandard::XRechnung;
  }
  match syntax {
    XmlSyntax::Ubl => EInvoiceStandard::UBL,
    XmlSyntax::Cii => {
      if id.contains("factur-x") || name.starts_with("factur-x") {
        EInvoiceStandard::FacturX
      } else if id.contains("zugferd") || name.contains("zugferd") {
        EInvoiceStandard::ZUGFeRD
      } else {
        EInvoiceStandard::Unknown
      }
    }
  }
}

/// Find the first attachment that parses as a CII or UBL invoice document.
/// Candidates with an e-invoice `/AFRelationship` or a well-known filename are
/// tried first, then anything else that looks like XML.
pub(crate) fn detect(
  attachments: &[RawAttachment],
  warnings: &mut Vec<String>,
) -> Option<DetectedXml> {
  let mut order: Vec<usize> = (0..attachments.len())
    .filter(|&i| is_xml_candidate(&attachments[i]))
    .collect();
  order.sort_by_key(|&i| {
    let a = &attachments[i];
    let rel = matches!(
      a.relationship.as_deref().unwrap_or(""),
      "Data" | "Alternative" | "Source"
    );
    let known = KNOWN_FILENAMES.contains(&a.name.to_ascii_lowercase().as_str());
    (!rel, !known)
  });

  for i in order {
    let a = &attachments[i];
    let Some(text) = xml_text(&a.data) else {
      warnings.push(format!(
        "attachment \"{}\" is not valid UTF-8; skipped",
        a.name
      ));
      continue;
    };
    let doc = match roxmltree::Document::parse(&text) {
      Ok(d) => d,
      Err(e) => {
        warnings.push(format!(
          "attachment \"{}\" is not well-formed XML: {e}",
          a.name
        ));
        continue;
      }
    };
    let root = doc.root_element();
    let local = root.tag_name().name();
    let ns = root.tag_name().namespace().unwrap_or("");
    let syntax = if local == "CrossIndustryInvoice" {
      XmlSyntax::Cii
    } else if (local == "Invoice" || local == "CreditNote") && ns.contains(UBL_NAMESPACE_MARKER) {
      XmlSyntax::Ubl
    } else {
      continue;
    };
    let identifier = match syntax {
      XmlSyntax::Cii => text_at(
        root,
        &[
          "ExchangedDocumentContext",
          "GuidelineSpecifiedDocumentContextParameter",
          "ID",
        ],
      ),
      XmlSyntax::Ubl => text_at(root, &["CustomizationID"]),
    };
    let standard = standard_from_identifier(identifier.as_deref().unwrap_or(""), &a.name, syntax);
    return Some(DetectedXml {
      attachment_index: i,
      syntax,
      standard,
      profile: identifier,
      xml: text,
    });
  }
  None
}

// ── Node helpers (local-name based) ───────────────────────────────

fn child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
  node
    .children()
    .find(|c| c.is_element() && c.tag_name().name() == name)
}

fn children<'a, 'i>(node: Node<'a, 'i>, name: &'a str) -> impl Iterator<Item = Node<'a, 'i>> + 'a {
  node
    .children()
    .filter(move |c| c.is_element() && c.tag_name().name() == name)
}

fn at<'a, 'i>(node: Node<'a, 'i>, path: &[&str]) -> Option<Node<'a, 'i>> {
  path.iter().try_fold(node, |n, name| child(n, name))
}

fn text(node: Node) -> Option<String> {
  let t = node.text()?.trim();
  (!t.is_empty()).then(|| t.to_string())
}

fn text_at(node: Node, path: &[&str]) -> Option<String> {
  at(node, path).and_then(text)
}

fn attr<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
  node.attribute(name)
}

// ── Field constructors ────────────────────────────────────────────

fn sf(text: String) -> StringField {
  StringField {
    value: text.clone(),
    text,
    confidence: 1.0,
    span: None,
  }
}

fn nf(text: String) -> Option<NumberField> {
  let value = text.trim().replace(',', ".").parse::<f64>().ok()?;
  Some(NumberField {
    value,
    text,
    confidence: 1.0,
    span: None,
  })
}

fn mf(text: String, currency: Option<&str>) -> Option<MoneyField> {
  let value = text.trim().replace(',', ".").parse::<f64>().ok()?;
  Some(MoneyField {
    value,
    text,
    confidence: 1.0,
    span: None,
    currency_code: currency.map(str::to_string),
    currency_symbol: None,
  })
}

/// CII `udt:DateTimeString` with `format="102"` is `YYYYMMDD`; UBL dates are ISO.
fn date_from_text(text: &str) -> Option<String> {
  let t = text.trim();
  let digits: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
  if t.len() >= 10 && t.as_bytes()[4] == b'-' && t.as_bytes()[7] == b'-' {
    return Some(t[..10].to_string());
  }
  if digits.len() == 8 {
    return Some(format!(
      "{}-{}-{}",
      &digits[..4],
      &digits[4..6],
      &digits[6..8]
    ));
  }
  None
}

fn df(text: String) -> Option<DateField> {
  let value = date_from_text(&text)?;
  Some(DateField {
    value,
    text,
    confidence: 1.0,
    span: None,
  })
}

fn address(lines: Vec<String>) -> Option<AddressField> {
  let lines: Vec<String> = lines
    .into_iter()
    .map(|l| l.trim().to_string())
    .filter(|l| !l.is_empty())
    .collect();
  if lines.is_empty() {
    return None;
  }
  let value = lines.join("\n");
  Some(AddressField {
    text: value.clone(),
    value,
    confidence: 1.0,
    span: None,
    lines,
  })
}

fn document_type_from_code(code: Option<&str>) -> InvoiceDocumentType {
  match code.map(str::trim) {
    Some("381") => InvoiceDocumentType::CreditNote,
    Some("380" | "384" | "389" | "875" | "876" | "877") => InvoiceDocumentType::Invoice,
    _ => InvoiceDocumentType::Unknown,
  }
}

// ── CII (UN/CEFACT Cross Industry Invoice D16B) ───────────────────

fn cii_date(node: Node, path: &[&str]) -> Option<DateField> {
  let dt = at(node, path)?;
  let s = child(dt, "DateTimeString").or(Some(dt))?;
  text(s).and_then(df)
}

fn cii_address(party: Node) -> Option<AddressField> {
  let addr = child(party, "PostalTradeAddress")?;
  let mut lines = Vec::new();
  for key in ["LineOne", "LineTwo", "LineThree"] {
    if let Some(l) = text_at(addr, &[key]) {
      lines.push(l);
    }
  }
  let post = text_at(addr, &["PostcodeCode"]).unwrap_or_default();
  let city = text_at(addr, &["CityName"]).unwrap_or_default();
  let city_line = format!("{post} {city}").trim().to_string();
  if !city_line.is_empty() {
    lines.push(city_line);
  }
  if let Some(c) = text_at(addr, &["CountryID"]) {
    lines.push(c);
  }
  address(lines)
}

fn cii_tax_id(party: Node) -> Option<String> {
  let regs: Vec<Node> = children(party, "SpecifiedTaxRegistration").collect();
  regs
    .iter()
    .find_map(|r| {
      let id = child(*r, "ID")?;
      (attr(id, "schemeID") == Some("VA"))
        .then(|| text(id))
        .flatten()
    })
    .or_else(|| regs.iter().find_map(|r| text_at(*r, &["ID"])))
}

pub(crate) fn map_cii(root: Node) -> (InvoiceFields, InvoiceDocumentType) {
  let mut f = InvoiceFields::default();
  let doc_type =
    document_type_from_code(text_at(root, &["ExchangedDocument", "TypeCode"]).as_deref());

  if let Some(exchanged) = child(root, "ExchangedDocument") {
    f.invoice_id = text_at(exchanged, &["ID"]).map(sf);
    f.invoice_date = cii_date(exchanged, &["IssueDateTime"]);
  }

  let Some(tx) = child(root, "SupplyChainTradeTransaction") else {
    return (f, doc_type);
  };

  if let Some(agreement) = child(tx, "ApplicableHeaderTradeAgreement") {
    if let Some(seller) = child(agreement, "SellerTradeParty") {
      f.vendor_name = text_at(seller, &["Name"]).map(sf);
      f.vendor_address = cii_address(seller);
      f.vendor_tax_id = cii_tax_id(seller).map(sf);
      f.vendor_email = text_at(seller, &["URIUniversalCommunication", "URIID"])
        .or_else(|| {
          text_at(
            seller,
            &[
              "DefinedTradeContact",
              "EmailURIUniversalCommunication",
              "URIID",
            ],
          )
        })
        .map(sf);
      f.vendor_phone = text_at(
        seller,
        &[
          "DefinedTradeContact",
          "TelephoneUniversalCommunication",
          "CompleteNumber",
        ],
      )
      .map(sf);
    }
    if let Some(buyer) = child(agreement, "BuyerTradeParty") {
      f.customer_name = text_at(buyer, &["Name"]).map(sf);
      f.customer_id = text_at(buyer, &["ID"]).map(sf);
      f.customer_address = cii_address(buyer);
      f.customer_tax_id = cii_tax_id(buyer).map(sf);
    }
    f.purchase_order = text_at(
      agreement,
      &["BuyerOrderReferencedDocument", "IssuerAssignedID"],
    )
    .map(sf);
  }

  if let Some(delivery) = child(tx, "ApplicableHeaderTradeDelivery")
    && let Some(ship_to) = child(delivery, "ShipToTradeParty")
  {
    f.shipping_address = cii_address(ship_to);
  }

  if let Some(settlement) = child(tx, "ApplicableHeaderTradeSettlement") {
    f.currency_code = text_at(settlement, &["InvoiceCurrencyCode"]);
    let cur = f.currency_code.as_deref();

    if let Some(payee) = child(settlement, "PayeeTradeParty") {
      f.remittance_address = cii_address(payee);
    }
    if let Some(period) = child(settlement, "BillingSpecifiedPeriod") {
      f.service_start_date = cii_date(period, &["StartDateTime"]);
      f.service_end_date = cii_date(period, &["EndDateTime"]);
    }
    for means in children(settlement, "SpecifiedTradeSettlementPaymentMeans") {
      let iban = text_at(means, &["PayeePartyCreditorFinancialAccount", "IBANID"]);
      let account = text_at(
        means,
        &["PayeePartyCreditorFinancialAccount", "ProprietaryID"],
      );
      let swift = text_at(
        means,
        &["PayeeSpecifiedCreditorFinancialInstitution", "BICID"],
      );
      let bank = text_at(
        means,
        &["PayeeSpecifiedCreditorFinancialInstitution", "Name"],
      );
      if iban.is_some() || account.is_some() || swift.is_some() || bank.is_some() {
        f.payment_details.push(PaymentDetail {
          iban: iban.map(sf),
          swift: swift.map(sf),
          bank_name: bank.map(sf),
          account_number: account.map(sf),
          routing_number: None,
        });
      }
    }
    for tax in children(settlement, "ApplicableTradeTax") {
      f.tax_details.push(TaxDetail {
        rate: text_at(tax, &["RateApplicablePercent"]).and_then(nf),
        amount: text_at(tax, &["CalculatedAmount"]).and_then(|t| mf(t, cur)),
        base: text_at(tax, &["BasisAmount"]).and_then(|t| mf(t, cur)),
        category: text_at(tax, &["CategoryCode"]).map(sf),
      });
    }
    if let Some(terms) = child(settlement, "SpecifiedTradePaymentTerms") {
      f.payment_term = text_at(terms, &["Description"]).map(sf);
      f.due_date = cii_date(terms, &["DueDateDateTime"]);
    }
    if let Some(sum) = child(
      settlement,
      "SpecifiedTradeSettlementHeaderMonetarySummation",
    ) {
      f.sub_total = text_at(sum, &["LineTotalAmount"]).and_then(|t| mf(t, cur));
      f.total_discount = text_at(sum, &["AllowanceTotalAmount"]).and_then(|t| mf(t, cur));
      f.shipping_cost = text_at(sum, &["ChargeTotalAmount"]).and_then(|t| mf(t, cur));
      f.total_tax = text_at(sum, &["TaxTotalAmount"]).and_then(|t| mf(t, cur));
      f.invoice_total = text_at(sum, &["GrandTotalAmount"]).and_then(|t| mf(t, cur));
      f.amount_due = text_at(sum, &["DuePayableAmount"]).and_then(|t| mf(t, cur));
      f.previous_unpaid_balance = text_at(sum, &["TotalPrepaidAmount"])
        .and_then(|t| mf(t, cur))
        .filter(|m| m.value != 0.0);
    }
  }

  let cur = f.currency_code.clone();
  for line in children(tx, "IncludedSupplyChainTradeLineItem") {
    let mut item = InvoiceItem::default();
    if let Some(product) = child(line, "SpecifiedTradeProduct") {
      item.description = text_at(product, &["Name"]).map(sf);
      item.product_code = text_at(product, &["SellerAssignedID"])
        .or_else(|| text_at(product, &["GlobalID"]))
        .or_else(|| text_at(product, &["BuyerAssignedID"]))
        .map(sf);
    }
    item.unit_price = text_at(
      line,
      &[
        "SpecifiedLineTradeAgreement",
        "NetPriceProductTradePrice",
        "ChargeAmount",
      ],
    )
    .and_then(|t| mf(t, cur.as_deref()));
    if let Some(qty) = at(line, &["SpecifiedLineTradeDelivery", "BilledQuantity"]) {
      item.quantity = text(qty).and_then(nf);
      item.unit = attr(qty, "unitCode").map(|u| sf(u.to_string()));
    }
    if let Some(settle) = child(line, "SpecifiedLineTradeSettlement") {
      item.tax_rate =
        text_at(settle, &["ApplicableTradeTax", "RateApplicablePercent"]).and_then(nf);
      item.amount = text_at(
        settle,
        &[
          "SpecifiedTradeSettlementLineMonetarySummation",
          "LineTotalAmount",
        ],
      )
      .and_then(|t| mf(t, cur.as_deref()));
      item.discount = text_at(settle, &["SpecifiedTradeAllowanceCharge", "ActualAmount"])
        .and_then(|t| mf(t, cur.as_deref()));
    }
    f.items.push(item);
  }

  (f, doc_type)
}

// ── UBL 2.x (Invoice / CreditNote) ────────────────────────────────

fn ubl_address(party: Node) -> Option<AddressField> {
  let addr = child(party, "PostalAddress")?;
  let mut lines = Vec::new();
  for key in ["StreetName", "AdditionalStreetName"] {
    if let Some(l) = text_at(addr, &[key]) {
      lines.push(l);
    }
  }
  if let Some(line) = child(addr, "AddressLine").and_then(|l| text_at(l, &["Line"])) {
    lines.push(line);
  }
  let post = text_at(addr, &["PostalZone"]).unwrap_or_default();
  let city = text_at(addr, &["CityName"]).unwrap_or_default();
  let city_line = format!("{post} {city}").trim().to_string();
  if !city_line.is_empty() {
    lines.push(city_line);
  }
  if let Some(c) = text_at(addr, &["Country", "IdentificationCode"]) {
    lines.push(c);
  }
  address(lines)
}

fn ubl_party_name(party: Node) -> Option<String> {
  text_at(party, &["PartyName", "Name"])
    .or_else(|| text_at(party, &["PartyLegalEntity", "RegistrationName"]))
}

pub(crate) fn map_ubl(root: Node) -> (InvoiceFields, InvoiceDocumentType) {
  let mut f = InvoiceFields::default();
  let is_credit_note = root.tag_name().name() == "CreditNote";
  let doc_type = if is_credit_note {
    InvoiceDocumentType::CreditNote
  } else {
    match document_type_from_code(text_at(root, &["InvoiceTypeCode"]).as_deref()) {
      InvoiceDocumentType::Unknown => InvoiceDocumentType::Invoice,
      t => t,
    }
  };

  f.invoice_id = text_at(root, &["ID"]).map(sf);
  f.invoice_date = text_at(root, &["IssueDate"]).and_then(df);
  f.due_date = text_at(root, &["DueDate"])
    .or_else(|| text_at(root, &["PaymentMeans", "PaymentDueDate"]))
    .and_then(df);
  f.currency_code = text_at(root, &["DocumentCurrencyCode"]);
  let cur = f.currency_code.clone();
  f.purchase_order = text_at(root, &["OrderReference", "ID"]).map(sf);
  if let Some(period) = child(root, "InvoicePeriod") {
    f.service_start_date = text_at(period, &["StartDate"]).and_then(df);
    f.service_end_date = text_at(period, &["EndDate"]).and_then(df);
  }

  if let Some(party) = at(root, &["AccountingSupplierParty", "Party"]) {
    f.vendor_name = ubl_party_name(party).map(sf);
    f.vendor_address = ubl_address(party);
    f.vendor_tax_id = text_at(party, &["PartyTaxScheme", "CompanyID"]).map(sf);
    f.vendor_email = text_at(party, &["Contact", "ElectronicMail"]).map(sf);
    f.vendor_phone = text_at(party, &["Contact", "Telephone"]).map(sf);
    f.vendor_website = text_at(party, &["WebsiteURI"]).map(sf);
  }
  if let Some(party) = at(root, &["AccountingCustomerParty", "Party"]) {
    f.customer_name = ubl_party_name(party).map(sf);
    f.customer_address = ubl_address(party);
    f.customer_tax_id = text_at(party, &["PartyTaxScheme", "CompanyID"]).map(sf);
    f.customer_id = text_at(party, &["PartyIdentification", "ID"]).map(sf);
  }
  if let Some(delivery) = child(root, "Delivery")
    && let Some(loc) = child(delivery, "DeliveryLocation")
  {
    let addr = child(loc, "Address");
    if let Some(addr) = addr {
      let mut shell = InvoiceFields::default();
      // reuse the postal-address reader by wrapping the node like a party
      let lines: Vec<String> = [
        "StreetName",
        "AdditionalStreetName",
        "CityName",
        "PostalZone",
      ]
      .iter()
      .filter_map(|k| text_at(addr, &[k]))
      .collect();
      shell.shipping_address = address(lines);
      f.shipping_address = shell.shipping_address;
    }
  }

  for means in children(root, "PaymentMeans") {
    let iban = text_at(means, &["PayeeFinancialAccount", "ID"]);
    let bank = text_at(means, &["PayeeFinancialAccount", "Name"]);
    let swift = text_at(
      means,
      &["PayeeFinancialAccount", "FinancialInstitutionBranch", "ID"],
    );
    if iban.is_some() || bank.is_some() || swift.is_some() {
      f.payment_details.push(PaymentDetail {
        iban: iban.map(sf),
        swift: swift.map(sf),
        bank_name: bank.map(sf),
        account_number: None,
        routing_number: None,
      });
    }
  }
  if let Some(terms) = child(root, "PaymentTerms") {
    f.payment_term = text_at(terms, &["Note"]).map(sf);
  }

  for total in children(root, "TaxTotal") {
    let subtotals: Vec<Node> = children(total, "TaxSubtotal").collect();
    if subtotals.is_empty() {
      f.tax_details.push(TaxDetail {
        amount: text_at(total, &["TaxAmount"]).and_then(|t| mf(t, cur.as_deref())),
        ..TaxDetail::default()
      });
    }
    for sub in subtotals {
      f.tax_details.push(TaxDetail {
        rate: text_at(sub, &["TaxCategory", "Percent"]).and_then(nf),
        amount: text_at(sub, &["TaxAmount"]).and_then(|t| mf(t, cur.as_deref())),
        base: text_at(sub, &["TaxableAmount"]).and_then(|t| mf(t, cur.as_deref())),
        category: text_at(sub, &["TaxCategory", "ID"]).map(sf),
      });
    }
  }
  f.total_tax = f.tax_details.iter().filter_map(|t| t.amount.as_ref()).fold(
    None,
    |acc: Option<MoneyField>, m| {
      Some(match acc {
        None => m.clone(),
        Some(mut a) => {
          a.value += m.value;
          a.text = format!("{:.2}", a.value);
          a
        }
      })
    },
  );

  if let Some(sum) = child(root, "LegalMonetaryTotal") {
    f.sub_total = text_at(sum, &["LineExtensionAmount"]).and_then(|t| mf(t, cur.as_deref()));
    f.total_discount = text_at(sum, &["AllowanceTotalAmount"]).and_then(|t| mf(t, cur.as_deref()));
    f.shipping_cost = text_at(sum, &["ChargeTotalAmount"]).and_then(|t| mf(t, cur.as_deref()));
    f.invoice_total = text_at(sum, &["TaxInclusiveAmount"]).and_then(|t| mf(t, cur.as_deref()));
    f.amount_due = text_at(sum, &["PayableAmount"]).and_then(|t| mf(t, cur.as_deref()));
    f.previous_unpaid_balance = text_at(sum, &["PrepaidAmount"])
      .and_then(|t| mf(t, cur.as_deref()))
      .filter(|m| m.value != 0.0);
  }

  let line_name = if is_credit_note {
    "CreditNoteLine"
  } else {
    "InvoiceLine"
  };
  let qty_name = if is_credit_note {
    "CreditedQuantity"
  } else {
    "InvoicedQuantity"
  };
  for line in children(root, line_name) {
    let mut item = InvoiceItem::default();
    if let Some(it) = child(line, "Item") {
      item.description = text_at(it, &["Name"])
        .or_else(|| text_at(it, &["Description"]))
        .map(sf);
      item.product_code = text_at(it, &["SellersItemIdentification", "ID"])
        .or_else(|| text_at(it, &["StandardItemIdentification", "ID"]))
        .map(sf);
      item.tax_rate = text_at(it, &["ClassifiedTaxCategory", "Percent"]).and_then(nf);
    }
    if let Some(qty) = child(line, qty_name) {
      item.quantity = text(qty).and_then(nf);
      item.unit = attr(qty, "unitCode").map(|u| sf(u.to_string()));
    }
    item.unit_price = text_at(line, &["Price", "PriceAmount"]).and_then(|t| mf(t, cur.as_deref()));
    item.amount = text_at(line, &["LineExtensionAmount"]).and_then(|t| mf(t, cur.as_deref()));
    f.items.push(item);
  }

  (f, doc_type)
}

#[cfg(test)]
mod tests;
