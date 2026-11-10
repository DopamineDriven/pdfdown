use super::*;

#[test]
  fn cell_references_and_ranges() {
    assert_eq!(parse_cell_ref("A1"), Some((0, 0)));
    assert_eq!(parse_cell_ref("$C$3"), Some((2, 2)));
    assert_eq!(parse_cell_ref("XFD1048576"), Some((1_048_575, 16_383)));
    assert_eq!(parse_cell_ref("A0"), None);
    assert_eq!(parse_cell_ref("1A"), None);
    assert_eq!(parse_cell_ref(""), None);
    assert_eq!(cell_ref(1, 27), "AB2");
    assert_eq!(CellRange::parse("C3:A1").unwrap().a1(), "A1:C3");
    assert_eq!(CellRange::parse("B2").unwrap().a1(), "B2:B2");
    assert_eq!(CellRange::parse("A1:B2:C3"), None);
  }

  #[test]
  fn styles_count_every_xf_and_resolve_format_codes() {
    let xml = br##"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><numFmts count="1"><numFmt numFmtId="164" formatCode="#,##0.00&quot;/hr&quot;"/></numFmts><fonts count="2"><font><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="10"/><name val="Arial"/></font></fonts><cellXfs count="2"><xf numFmtId="4" fontId="1"><alignment vertical="center" wrapText="1" indent="1"/></xf><xf numFmtId="164" fontId="0"/></cellXfs></styleSheet>"##;
    let styles = parse_styles(xml).unwrap();
    assert_eq!(styles.xfs.len(), 2);
    assert_eq!(styles.xfs[0].vertical, Vertical::Center);
    assert!(styles.xfs[0].wrap);
    assert_eq!(styles.xfs[0].indent, 1);
    assert_eq!(styles.xfs[0].font_id, 1);
    assert!(styles.fonts[1].bold);
    assert_eq!(styles.fonts[1].name, "Arial");
    assert_eq!(format_code(&styles, 0).as_deref(), Some("#,##0.00"));
    assert_eq!(format_code(&styles, 1).as_deref(), Some("#,##0.00\"/hr\""));
    assert_eq!(format_code(&styles, 9), None);
  }

  fn sheet_of(xml: &[u8], styles: &Styles) -> Sheet {
    parse_sheet(
      xml,
      "S",
      0,
      false,
      WorkbookContext {
        shared: &[],
        styles,
        date1904: false,
      },
      None,
    )
    .unwrap()
  }

  #[test]
  fn references_beyond_the_grid_are_malformed_and_rows_never_wrap() {
    assert_eq!(parse_cell_ref("XFD1048576"), Some((1_048_575, 16_383)));
    assert_eq!(parse_cell_ref("A1048577"), None, "row past 1,048,576");
    assert_eq!(parse_cell_ref("XFE1"), None, "column past XFD");
    assert_eq!(parse_cell_ref("A999999999"), None);
    let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1048576"><c><v>1</v></c></row><row><c><v>2</v></c><c r="A999999999"><v>3</v></c></row><row r="4294967295"><c><v>4</v></c></row><row><c><v>5</v></c></row></sheetData></worksheet>"##;
    let sheet = sheet_of(xml, &Styles::default());
    assert_eq!(
      sheet.cells.keys().collect::<Vec<_>>(),
      [&(1_048_575, 0)],
      "only the last row of the grid holds a cell: {:?}",
      sheet.warnings
    );
    assert!(sheet.cells.values().all(|c| c.text == "1"));
    assert!(
      sheet
        .warnings
        .iter()
        .filter(|w| w.contains("beyond"))
        .count()
        >= 3,
      "{:?}",
      sheet.warnings
    );
  }

  #[test]
  fn general_cells_render_through_general_and_values_are_finite() {
    let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1"><v>0.30000000000000004</v></c><c r="B1"><v>427.49999999999994</v></c><c r="C1"><v>1e300</v></c><c r="D1"><v>NaN</v></c><c r="E1"><v>inf</v></c></row></sheetData></worksheet>"##;
    let sheet = sheet_of(xml, &Styles::default());
    assert_eq!(sheet.cells[&(0, 0)].text, "0.3");
    assert_eq!(sheet.cells[&(0, 0)].raw, "0.30000000000000004");
    assert_eq!(sheet.cells[&(0, 1)].text, "427.5");
    assert_eq!(sheet.cells[&(0, 2)].text, "1E+300");
    assert_eq!(
      sheet.cells[&(0, 3)].value,
      CellValue::Text("NaN".to_string())
    );
    assert_eq!(
      sheet.cells[&(0, 4)].value,
      CellValue::Text("inf".to_string())
    );
  }

  #[test]
  fn builtin_ids_render_or_are_unsupported_and_a_files_own_code_wins() {
    let xml = br##"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><numFmts count="1"><numFmt numFmtId="4" formatCode="0.0"/></numFmts><cellXfs count="4"><xf numFmtId="44"/><xf numFmtId="27"/><xf numFmtId="4"/><xf numFmtId="7"/></cellXfs></styleSheet>"##;
    let styles = parse_styles(xml).unwrap();
    assert_eq!(
      format_code(&styles, 0).as_deref(),
      Some("_($* #,##0.00_);_($* (#,##0.00);_($* \"-\"??_);_(@_)")
    );
    assert_eq!(
      format_code(&styles, 2).as_deref(),
      Some("0.0"),
      "the file's own code for id 4 wins"
    );
    assert_eq!(
      format_code(&styles, 3).as_deref(),
      Some("$#,##0.00_);($#,##0.00)")
    );
    let sheet_xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" s="0"><v>1234.5</v></c><c r="B1" s="1"><v>42247</v></c><c r="C1" s="1"><v>7</v></c></row></sheetData></worksheet>"##;
    let sheet = sheet_of(sheet_xml, &styles);
    assert_eq!(sheet.cells[&(0, 0)].text, "$1,234.50");
    let unknown = &sheet.cells[&(0, 1)];
    assert!(
      !unknown.format_supported,
      "built-in 27 has no en-US code this release knows"
    );
    assert_eq!(unknown.text, "42247");
    assert_eq!(unknown.value, CellValue::Number(42247.0));
    assert!(
      sheet
        .warnings
        .iter()
        .any(|w| w.contains("built-in") && w.contains("27")),
      "{:?}",
      sheet.warnings
    );
  }

  #[test]
  fn a_multi_range_print_area_is_no_print_area() {
    assert_eq!(print_area_range("S!$A$1:$B$2,S!$D$1:$E$2"), None);
    assert_eq!(
      print_area_range("'My Sheet'!$A$1:$F$12").unwrap().a1(),
      "A1:F12"
    );
  }

  #[test]
  fn geometry_from_the_file_is_finite_and_within_excels_maxima() {
    let styles_xml = br##"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="2"><font><sz val="1e999"/></font><font><sz val="NaN"/></font></fonts><cellXfs count="1"><xf numFmtId="0" fontId="0"/></cellXfs></styleSheet>"##;
    let styles = parse_styles(styles_xml).unwrap();
    assert_eq!(styles.fonts[0].size, 409.0);
    assert_eq!(styles.fonts[1].size, 11.0);
    let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetFormatPr defaultRowHeight="inf" defaultColWidth="-5"/><cols><col min="1" max="1" width="1e999"/><col min="2" max="2" width="NaN"/></cols><sheetData><row r="1" ht="1e999"><c r="A1"><v>1</v></c></row><row r="2" ht="-3"><c r="A2"><v>2</v></c></row></sheetData></worksheet>"##;
    let sheet = sheet_of(xml, &styles);
    assert_eq!(
      sheet.default_row_height, 409.5,
      "an overflowing spelling is the maximum"
    );
    assert_eq!(
      sheet.default_col_width, None,
      "a negative width is no width"
    );
    assert_eq!(sheet.cols[0].width, 255.0);
    assert_eq!(
      sheet.cols[1].width, 0.0,
      "an unparsable width means the default"
    );
    assert_eq!(sheet.rows[&0].height, Some(409.5));
    assert_eq!(sheet.rows.get(&1).and_then(|r| r.height), None);
  }

  #[test]
  fn time_only_codes_type_the_cell_as_time_and_numbers_carry_subtype_and_currency() {
    use crate::types::{NumberSubtype, SheetCell};
    let styles_xml = br##"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><numFmts count="1"><numFmt numFmtId="164" formatCode="[$&#8364;-407]#,##0.00"/></numFmts><cellXfs count="5"><xf numFmtId="0"/><xf numFmtId="20"/><xf numFmtId="22"/><xf numFmtId="164"/><xf numFmtId="4"/></cellXfs></styleSheet>"##;
    let styles = parse_styles(styles_xml).unwrap();
    let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" s="1"><v>0.75</v></c><c r="B1" s="2"><v>45161.5</v></c><c r="C1" s="3"><v>1234.5</v></c><c r="D1" s="4"><v>3</v></c><c r="E1"><v>2.5</v></c></row></sheetData></worksheet>"##;
    let sheet = sheet_of(xml, &styles);
    assert_eq!(
      sheet.cells[&(0, 0)].value,
      CellValue::Time("18:00:00".to_string())
    );
    assert_eq!(sheet.cells[&(0, 0)].text, "18:00");
    assert_eq!(
      sheet.cells[&(0, 1)].value,
      CellValue::Date("2023-08-23T12:00:00".to_string())
    );
    let workbook = Workbook {
      sheets: vec![sheet],
      date1904: false,
      styles,
    };
    let cells = &cells_of(&workbook, &SpreadsheetOptions::default())[0].cells;
    let number = |r#ref: &str| match cells
      .iter()
      .find(|c| matches!(c, SheetCell::Number { r#ref: x, .. } if x == r#ref))
    {
      Some(SheetCell::Number {
        subtype,
        currency_symbol,
        ..
      }) => (*subtype, currency_symbol.clone()),
      other => panic!("{ref}: not a Number: {other:?}"),
    };
    assert_eq!(number("C1"), (NumberSubtype::Float, Some("€".to_string())));
    assert_eq!(number("D1"), (NumberSubtype::Integer, None));
    assert_eq!(number("E1"), (NumberSubtype::Float, None));
    assert!(
      matches!(cells.iter().find(|c| matches!(c, SheetCell::Time { .. })), Some(SheetCell::Time { value, .. }) if value == "18:00:00")
    );
  }

  #[test]
  fn malformed_references_indices_and_merges_warn_and_skip() {
    let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="ZZZZ"><v>1</v></c><c r="B1" t="s"><v>99</v></c><c r="C1" t="s"><v>1</v></c></row></sheetData><mergeCells><mergeCell ref="nope"/></mergeCells></worksheet>"##;
    let shared = vec!["zero".to_string(), "one".to_string()];
    let sheet = parse_sheet(
      xml,
      "S",
      0,
      false,
      WorkbookContext {
        shared: &shared,
        styles: &Styles::default(),
        date1904: false,
      },
      None,
    )
    .unwrap();
    assert_eq!(sheet.warnings.len(), 3, "{:?}", sheet.warnings);
    assert_eq!(sheet.cells.len(), 1);
    assert_eq!(sheet.cells[&(0, 2)].text, "one");
  }
