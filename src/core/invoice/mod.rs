//! Invoice extraction orchestration. Track 1 (this version): embedded
//! e-invoice XML. Later tracks add layout-based heuristics over native text
//! and OCR; they will fill the same `InvoiceResult` shape.

pub(crate) mod einvoice;

use crate::core::attachments::extract_attachments;
use crate::types::{
  EmbeddedInvoiceXml, InvoiceDocumentType, InvoiceFields, InvoiceOptions, InvoiceResult,
  InvoiceSource,
};
use lopdf::Document;

pub(crate) const NO_SOURCE_WARNING: &str = "no embedded e-invoice XML (Factur-X / ZUGFeRD / XRechnung / UBL) was found; \
   layout-based extraction is not available in this version";

pub(crate) fn extract_invoice(doc: &Document, opts: Option<&InvoiceOptions>) -> InvoiceResult {
  let mut warnings = Vec::new();
  let prefer_xml = opts.and_then(|o| o.prefer_embedded_xml).unwrap_or(true);

  if prefer_xml {
    let attachments = extract_attachments(doc);
    if let Some(found) = einvoice::detect(&attachments, &mut warnings) {
      // The text parsed once in `detect`; parse again here to borrow the tree locally.
      match roxmltree::Document::parse(&found.xml) {
        Ok(xml_doc) => {
          let root = xml_doc.root_element();
          let (fields, document_type) = match found.syntax {
            einvoice::XmlSyntax::Cii => einvoice::map_cii(root),
            einvoice::XmlSyntax::Ubl => einvoice::map_ubl(root),
          };
          return InvoiceResult {
            source: InvoiceSource::EmbeddedXml,
            document_type,
            confidence: 1.0,
            fields,
            warnings,
            embedded_xml: Some(EmbeddedInvoiceXml {
              filename: attachments[found.attachment_index].name.clone(),
              standard: found.standard,
              profile: found.profile,
              xml: found.xml,
            }),
          };
        }
        Err(e) => warnings.push(format!("embedded XML could not be re-parsed: {e}")),
      }
    }
  } else {
    warnings.push("preferEmbeddedXml is false; embedded XML was not consulted".to_string());
  }

  warnings.push(NO_SOURCE_WARNING.to_string());
  InvoiceResult {
    source: InvoiceSource::None,
    document_type: InvoiceDocumentType::Unknown,
    confidence: 0.0,
    fields: InvoiceFields::default(),
    warnings,
    embedded_xml: None,
  }
}
