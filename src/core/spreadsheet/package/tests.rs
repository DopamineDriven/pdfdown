use super::*;

fn fixture(name: &str) -> Vec<u8> {
  std::fs::read(format!("__test__/fixtures/spreadsheet/{name}")).unwrap()
}

#[test]
fn detects_pdf_workbook_and_the_unsupported_kinds() {
  assert_eq!(detect(b"%PDF-1.7\n").format, DocumentFormat::Pdf);
  assert_eq!(
    detect(&fixture("minimal.xlsx")).format,
    DocumentFormat::Xlsx
  );
  let d = detect(&fixture("not-a-workbook.docx"));
  assert_eq!(d.format, DocumentFormat::Unknown);
  assert_eq!(d.detail.as_deref(), Some("Word document package"));
  assert_eq!(
    detect(&fixture("macro.xlsm")).detail.as_deref(),
    Some("macro-enabled workbook (not supported in this release)")
  );
  assert_eq!(
    detect(&fixture("strict.xlsx")).detail.as_deref(),
    Some("Strict OOXML workbook (not supported in this release)")
  );
  assert_eq!(
    detect(&fixture("ole.bin")).detail.as_deref(),
    Some("OLE container (legacy .xls or an encrypted Office file)")
  );
  assert_eq!(
    detect(b"hello").detail.as_deref(),
    Some("not a PDF or a zip package")
  );
  // Files a document store hands out under a .pdf name that are something else entirely.
  assert_eq!(
    detect(b"II*\x00\x08\x00\x00\x00").detail.as_deref(),
    Some("TIFF image")
  );
  assert_eq!(
    detect(b"MM\x00*\x00\x00\x00\x08").detail.as_deref(),
    Some("TIFF image")
  );
  assert_eq!(
    detect(b"{\\rtf1\\ansi").detail.as_deref(),
    Some("RTF document")
  );
  assert_eq!(
    detect(b"\xff\xd8\xff\xe0").detail.as_deref(),
    Some("JPEG image")
  );
  assert_eq!(
    detect(b"\x89PNG\r\n\x1a\n").detail.as_deref(),
    Some("PNG image")
  );
}

#[test]
fn a_part_past_the_cap_is_a_limit_error_not_an_allocation() {
  let mut package = Package::open_with_limits(
    fixture("minimal.xlsx"),
    Limits {
      part_bytes: 64,
      ..Limits::default()
    },
  )
  .unwrap();
  assert!(matches!(
    package.part("xl/workbook.xml"),
    Err(SpreadsheetError::LimitExceeded(_))
  ));
  let mut package = Package::open(fixture("minimal.xlsx")).unwrap();
  assert!(
    package
      .part("xl/workbook.xml")
      .unwrap()
      .starts_with(b"<?xml")
  );
  assert!(matches!(
    package.part("xl/missing.xml"),
    Err(SpreadsheetError::Malformed(_))
  ));
  assert_eq!(package.officedocument().unwrap(), "xl/workbook.xml");
}

#[test]
fn relationship_targets_resolve_against_their_part() {
  assert_eq!(
    resolve_target("xl/", "worksheets/sheet1.xml"),
    "xl/worksheets/sheet1.xml"
  );
  assert_eq!(resolve_target("xl/", "/xl/styles.xml"), "xl/styles.xml");
  assert_eq!(
    resolve_target("xl/worksheets/", "../drawings/d1.xml"),
    "xl/drawings/d1.xml"
  );
  assert_eq!(rels_base("_rels/.rels"), "");
  assert_eq!(rels_base("xl/_rels/workbook.xml.rels"), "xl/");
}
