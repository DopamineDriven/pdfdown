//! Invoice extraction orchestration: track 1 reads embedded e-invoice XML; track 2, the
//! layout track in `layout.rs` and the modules beside it, reads the pages when there is none.

pub(crate) mod amounts;
pub(crate) mod anchors;
pub(crate) mod dates;
pub(crate) mod einvoice;
pub(crate) mod identifiers;
pub(crate) mod layout;
pub(crate) mod lexicon;
pub(crate) mod parties;
pub(crate) mod priors;
pub(crate) mod segments;
#[cfg(test)]
pub(crate) mod testing;

use crate::core::attachments::extract_attachments;
use crate::types::{EmbeddedInvoiceXml, InvoiceOptions, InvoiceResult, InvoiceSource, PageLayout};
use lopdf::Document;

pub(crate) const NO_XML_WARNING: &str = "no embedded e-invoice XML (Factur-X / ZUGFeRD / XRechnung / UBL) was found; the pages were read instead";

/// Embedded XML first when preferred, else the layout track over `pages`.
pub(crate) fn extract_invoice(
  doc: &Document,
  opts: Option<&InvoiceOptions>,
  pages: &[PageLayout],
) -> InvoiceResult {
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
            currency_source: None,
            credit_signals: Vec::new(),
          };
        }
        Err(e) => warnings.push(format!("embedded XML could not be re-parsed: {e}")),
      }
    }
  } else {
    warnings.push("preferEmbeddedXml is false; embedded XML was not consulted".to_string());
  }

  if prefer_xml {
    warnings.push(NO_XML_WARNING.to_string());
  }
  let mut result = layout::extract_from_pages(pages, opts);
  warnings.append(&mut result.warnings);
  result.warnings = warnings;
  result
}
