//! Bounded access to an OOXML package and format detection.
//!
//! A package is a zip; its parts are found through relationships (`_rels/.rels` names the main
//! part, each part's own `.rels` names what it refers to) and typed by `[Content_Types].xml`.
//! Every read is capped: the input, the entry count, each inflated part and their total.

use std::io::{Cursor, Read};

use roxmltree::{Document as XmlDocument, Node, ParsingOptions};
use zip::ZipArchive;

use crate::types::{DocumentFormat, FormatDetection};

/// Caps enforced on bytes actually read, never on what the zip headers claim.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Limits {
  pub input_bytes: usize,
  pub entries: usize,
  pub part_bytes: usize,
  pub total_bytes: usize,
}

impl Default for Limits {
  fn default() -> Self {
    Limits {
      input_bytes: 64 << 20,
      entries: 10_000,
      part_bytes: 64 << 20,
      total_bytes: 256 << 20,
    }
  }
}

/// XML nodes a single part may hold; roxmltree's default is effectively unbounded.
pub(crate) const XML_NODES_LIMIT: u32 = 5_000_000;

#[derive(Debug)]
pub(crate) enum SpreadsheetError {
  UnsupportedFormat(String),
  LimitExceeded(String),
  Malformed(String),
}

impl std::fmt::Display for SpreadsheetError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      SpreadsheetError::UnsupportedFormat(d) => write!(f, "unsupported format: {d}"),
      SpreadsheetError::LimitExceeded(d) => write!(f, "limit exceeded: {d}"),
      SpreadsheetError::Malformed(d) => write!(f, "malformed workbook: {d}"),
    }
  }
}

impl From<SpreadsheetError> for napi::Error {
  fn from(error: SpreadsheetError) -> napi::Error {
    napi::Error::from_reason(error.to_string())
  }
}

/// What the main part of a package says it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PackageKind {
  Workbook,
  MacroWorkbook,
  TemplateWorkbook,
  StrictWorkbook,
  WordDocument,
  Presentation,
  Other,
}

impl PackageKind {
  /// Why a package other than a plain workbook is not opened.
  pub(crate) fn detail(self) -> &'static str {
    match self {
      PackageKind::Workbook => "workbook",
      PackageKind::MacroWorkbook => "macro-enabled workbook (not supported in this release)",
      PackageKind::TemplateWorkbook => "template workbook (not supported in this release)",
      PackageKind::StrictWorkbook => "Strict OOXML workbook (not supported in this release)",
      PackageKind::WordDocument => "Word document package",
      PackageKind::Presentation => "PowerPoint presentation package",
      PackageKind::Other => "zip package that is not an Office document",
    }
  }
}

/// One relationship: `target` is resolved to an absolute part path unless it is external.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Relationship {
  pub id: String,
  pub kind: String,
  pub target: String,
  pub external: bool,
}

const WORKBOOK_MAIN: &str =
  "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml";
const MACRO_WORKBOOK_MAIN: &str = "application/vnd.ms-excel.sheet.macroEnabled.main+xml";
const TEMPLATE_MAINS: [&str; 2] = [
  "application/vnd.openxmlformats-officedocument.spreadsheetml.template.main+xml",
  "application/vnd.ms-excel.template.macroEnabled.main+xml",
];
const WORD_PREFIX: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml";
const PRESENTATION_PREFIX: &str = "application/vnd.openxmlformats-officedocument.presentationml";
const SML_STRICT: &str = "http://purl.oclc.org/ooxml/spreadsheetml/main";
const OLE_SIGNATURE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
/// The detail for a buffer that is neither; such a buffer still goes to lopdf, which can read a PDF
/// whose header is not at offset 0.
pub(crate) const NOT_PDF_OR_ZIP: &str = "not a PDF or a zip package";

/// Parse one part's XML under the node limit, with DTDs and external entities off (roxmltree's default).
pub(crate) fn parse_xml(bytes: &[u8]) -> Result<XmlDocument<'_>, SpreadsheetError> {
  let text = std::str::from_utf8(bytes)
    .map_err(|e| SpreadsheetError::Malformed(format!("part is not UTF-8: {e}")))?;
  XmlDocument::parse_with_options(
    text,
    ParsingOptions {
      nodes_limit: XML_NODES_LIMIT,
      ..ParsingOptions::default()
    },
  )
  .map_err(|e| SpreadsheetError::Malformed(format!("part is not well-formed XML: {e}")))
}

fn attr<'a>(node: Node<'a, 'a>, name: &str) -> &'a str {
  node.attribute(name).unwrap_or("")
}

/// The directory a `.rels` part describes: `xl/_rels/workbook.xml.rels` → `xl/`, `_rels/.rels` → ``.
fn rels_base(rels_path: &str) -> &str {
  rels_path
    .rsplit_once("_rels/")
    .map(|(dir, _)| dir)
    .unwrap_or("")
}

/// Join a relationship target onto its base and normalise `.` and `..`; a leading `/` is absolute.
fn resolve_target(base: &str, target: &str) -> String {
  let joined = match target.strip_prefix('/') {
    Some(absolute) => absolute.to_string(),
    None => format!("{base}{target}"),
  };
  let mut segments: Vec<&str> = Vec::new();
  for segment in joined.split('/') {
    match segment {
      "" | "." => {}
      ".." => {
        segments.pop();
      }
      other => segments.push(other),
    }
  }
  segments.join("/")
}

/// An open package: the zip archive plus the running count of bytes inflated so far.
pub(crate) struct Package {
  archive: ZipArchive<Cursor<Vec<u8>>>,
  limits: Limits,
  read_total: usize,
}

impl Package {
  pub(crate) fn open(bytes: Vec<u8>) -> Result<Package, SpreadsheetError> {
    Package::open_with_limits(bytes, Limits::default())
  }

  pub(crate) fn open_with_limits(
    bytes: Vec<u8>,
    limits: Limits,
  ) -> Result<Package, SpreadsheetError> {
    if bytes.len() > limits.input_bytes {
      return Err(SpreadsheetError::LimitExceeded(format!(
        "the package is {} bytes; the limit is {}",
        bytes.len(),
        limits.input_bytes
      )));
    }
    let archive = ZipArchive::new(Cursor::new(bytes))
      .map_err(|e| SpreadsheetError::Malformed(format!("not a readable zip package: {e}")))?;
    if archive.len() > limits.entries {
      return Err(SpreadsheetError::LimitExceeded(format!(
        "the package has {} entries; the limit is {}",
        archive.len(),
        limits.entries
      )));
    }
    Ok(Package {
      archive,
      limits,
      read_total: 0,
    })
  }

  /// One part, inflated, or a limit error once it passes the per-part or total cap.
  pub(crate) fn part(&mut self, path: &str) -> Result<Vec<u8>, SpreadsheetError> {
    let limit = self.limits.part_bytes;
    let total_limit = self.limits.total_bytes;
    let mut file = self
      .archive
      .by_name(path)
      .map_err(|_| SpreadsheetError::Malformed(format!("part {path} is missing")))?;
    let mut bytes = Vec::new();
    file
      .by_ref()
      .take(limit as u64 + 1)
      .read_to_end(&mut bytes)
      .map_err(|e| SpreadsheetError::Malformed(format!("part {path} could not be read: {e}")))?;
    if bytes.len() > limit {
      return Err(SpreadsheetError::LimitExceeded(format!(
        "part {path} inflates past {limit} bytes"
      )));
    }
    self.read_total += bytes.len();
    if self.read_total > total_limit {
      return Err(SpreadsheetError::LimitExceeded(format!(
        "the package's parts inflate past {total_limit} bytes in total"
      )));
    }
    Ok(bytes)
  }

  /// The relationships a `.rels` part declares, targets resolved against the part it describes.
  pub(crate) fn relationships(
    &mut self,
    rels_path: &str,
  ) -> Result<Vec<Relationship>, SpreadsheetError> {
    let bytes = self.part(rels_path)?;
    let doc = parse_xml(&bytes)?;
    let base = rels_base(rels_path);
    Ok(
      doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "Relationship")
        .map(|n| {
          let external = n.attribute("TargetMode") == Some("External");
          let target = attr(n, "Target");
          Relationship {
            id: attr(n, "Id").to_string(),
            kind: attr(n, "Type").to_string(),
            target: if external {
              target.to_string()
            } else {
              resolve_target(base, target)
            },
            external,
          }
        })
        .collect(),
    )
  }

  /// The main part named by the package relationships.
  pub(crate) fn officedocument(&mut self) -> Result<String, SpreadsheetError> {
    self
      .relationships("_rels/.rels")?
      .into_iter()
      .find(|r| !r.external && r.kind.ends_with("/officeDocument"))
      .map(|r| r.target)
      .ok_or_else(|| SpreadsheetError::Malformed("no officeDocument relationship".to_string()))
  }

  /// A part's content type from `[Content_Types].xml`: an override by name, else a default by extension.
  pub(crate) fn content_type(&mut self, part: &str) -> Result<Option<String>, SpreadsheetError> {
    let bytes = self.part("[Content_Types].xml")?;
    let doc = parse_xml(&bytes)?;
    let wanted = format!("/{part}");
    let extension = part.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    let mut by_default = None;
    for n in doc.descendants().filter(|n| n.is_element()) {
      match n.tag_name().name() {
        "Override" if attr(n, "PartName") == wanted => {
          return Ok(Some(attr(n, "ContentType").to_string()));
        }
        "Default" if attr(n, "Extension").eq_ignore_ascii_case(extension) => {
          by_default = Some(attr(n, "ContentType").to_string());
        }
        _ => {}
      }
    }
    Ok(by_default)
  }

  /// What the package is, by its main part's content type and, for workbooks, its namespace.
  pub(crate) fn kind(&mut self) -> Result<PackageKind, SpreadsheetError> {
    let main = self.officedocument()?;
    let content_type = self.content_type(&main)?.unwrap_or_default();
    Ok(match content_type.as_str() {
      WORKBOOK_MAIN => {
        let bytes = self.part(&main)?;
        let doc = parse_xml(&bytes)?;
        if doc.root_element().tag_name().namespace() == Some(SML_STRICT) {
          PackageKind::StrictWorkbook
        } else {
          PackageKind::Workbook
        }
      }
      MACRO_WORKBOOK_MAIN => PackageKind::MacroWorkbook,
      t if TEMPLATE_MAINS.contains(&t) => PackageKind::TemplateWorkbook,
      t if t.starts_with(WORD_PREFIX) => PackageKind::WordDocument,
      t if t.starts_with(PRESENTATION_PREFIX) => PackageKind::Presentation,
      _ => PackageKind::Other,
    })
  }
}

/// `Pdf` by header, `Xlsx` by a workbook main part behind the package relationships, else
/// `Unknown` with what the input was instead.
/// A `%PDF-` header within the first KiB, where lopdf also looks for it.
pub(crate) fn has_pdf_header(bytes: &[u8]) -> bool {
  bytes[..bytes.len().min(1024)]
    .windows(5)
    .any(|w| w == b"%PDF-")
}

/// The zip local-file-header signature.
pub(crate) const ZIP_MAGIC: &[u8; 4] = b"PK\x03\x04";

pub(crate) fn detect(bytes: &[u8]) -> FormatDetection {
  let unknown = |detail: String| FormatDetection {
    format: DocumentFormat::Unknown,
    detail: Some(detail),
  };
  if has_pdf_header(bytes) {
    return FormatDetection {
      format: DocumentFormat::Pdf,
      detail: None,
    };
  }
  if bytes.starts_with(ZIP_MAGIC) {
    return match Package::open(bytes.to_vec()).and_then(|mut p| p.kind()) {
      Ok(PackageKind::Workbook) => FormatDetection {
        format: DocumentFormat::Xlsx,
        detail: None,
      },
      Ok(kind) => unknown(kind.detail().to_string()),
      Err(SpreadsheetError::LimitExceeded(d)) => unknown(d),
      Err(_) => unknown(PackageKind::Other.detail().to_string()),
    };
  }
  if bytes.starts_with(&OLE_SIGNATURE) {
    return unknown("OLE container (legacy .xls or an encrypted Office file)".to_string());
  }
  // Document stores hand these out under .pdf names; naming them saves the caller a parse error.
  let other = [
    (&b"II*\x00"[..], "TIFF image"),
    (&b"MM\x00*"[..], "TIFF image"),
    (&b"{\\rtf"[..], "RTF document"),
    (&b"\xff\xd8\xff"[..], "JPEG image"),
    (&b"\x89PNG\r\n\x1a\n"[..], "PNG image"),
  ];
  if let Some((_, detail)) = other.iter().find(|(magic, _)| bytes.starts_with(magic)) {
    return unknown((*detail).to_string());
  }
  unknown(NOT_PDF_OR_ZIP.to_string())
}

#[cfg(test)]
mod tests;
