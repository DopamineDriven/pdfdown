use super::*;
use lopdf::{Stream, dictionary};

#[test]
fn subset_prefix_is_stripped_only_when_well_formed() {
  assert_eq!(
    strip_subset_prefix("ABCDEF+Helvetica-Bold"),
    "Helvetica-Bold"
  );
  assert_eq!(strip_subset_prefix("Helvetica+Bold"), "Helvetica+Bold");
  assert_eq!(strip_subset_prefix("Helvetica"), "Helvetica");
}

#[test]
fn standard14_font_without_widths_uses_builtin_metrics() {
  let doc = Document::with_version("1.7");
  let font = dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica-Bold" };
  let f = load_font(&doc, &font);
  assert_eq!(f.width_source, WidthSource::Standard14);
  assert!(f.bold);
  assert_eq!(f.width(b'A' as u32), 0.722);
  assert_eq!(f.width(200), 0.5, "outside the table: default half em");
  assert_eq!(f.decode(b"A"), "A");
  assert!((f.ascent - 0.718).abs() < 1e-9);
}

#[test]
fn simple_font_widths_and_descriptor_flags() {
  let mut doc = Document::with_version("1.7");
  let desc = doc.add_object(dictionary! {
      "Type" => "FontDescriptor", "Flags" => 1 << 18, "Ascent" => 900, "Descent" => -250, "MissingWidth" => 400,
    });
  let font = dictionary! {
    "Type" => "Font", "Subtype" => "TrueType", "BaseFont" => "ABCDEF+Lato",
    "FirstChar" => 65, "Widths" => vec![600.into(), 700.into()], "FontDescriptor" => Object::Reference(desc),
  };
  let f = load_font(&doc, &font);
  assert_eq!(f.name, "Lato");
  assert_eq!(f.width_source, WidthSource::Metric);
  assert!(f.bold, "ForceBold flag");
  assert!(!f.italic);
  assert_eq!(f.width(65), 0.6);
  assert_eq!(f.width(66), 0.7);
  assert_eq!(f.width(67), 0.4, "MissingWidth");
  assert!((f.ascent - 0.9).abs() < 1e-9 && (f.descent + 0.25).abs() < 1e-9);
}

#[test]
fn barcode_font_is_symbolic() {
  let doc = Document::with_version("1.7");
  let dict =
    dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "IDAutomationHC39M" };
  let f = load_font(&doc, &dict);
  assert!(f.symbolic, "barcode glyphs are bars, not letters");
  assert!(
    f.weight_known,
    "a named font has a known weight even when it is regular"
  );
  assert!(!f.bold);
  let dict = dictionary! { "Type" => "Font", "Subtype" => "TrueType", "BaseFont" => "LibreBarcode128-Regular" };
  assert!(load_font(&doc, &dict).symbolic);
  let dict = dictionary! { "Type" => "Font", "Subtype" => "TrueType", "BaseFont" => "UPC-A" };
  assert!(load_font(&doc, &dict).symbolic);
  let dict =
    dictionary! { "Type" => "Font", "Subtype" => "TrueType", "BaseFont" => "Cupcake-Regular" };
  assert!(
    !load_font(&doc, &dict).symbolic,
    "a word that merely contains \"upc\" is not a barcode font"
  );
  let dict =
    dictionary! { "Type" => "Font", "Subtype" => "TrueType", "BaseFont" => "Free3of9Extended" };
  assert!(load_font(&doc, &dict).symbolic);
}

#[test]
fn anonymous_cid_font_has_unknown_weight() {
  // Microsoft Print To PDF names its fonts CIDFont+F1, CIDFont+F2, …: nothing about weight or style.
  let doc = Document::with_version("1.7");
  let dict = dictionary! { "Type" => "Font", "Subtype" => "TrueType", "BaseFont" => "CIDFont+F1" };
  let f = load_font(&doc, &dict);
  assert!(!f.weight_known);
  assert!(!f.symbolic);
  let dict =
    dictionary! { "Type" => "Font", "Subtype" => "TrueType", "BaseFont" => "ABCDEE+CIDFont+F3" };
  assert!(
    !load_font(&doc, &dict).weight_known,
    "subset prefix stripped first"
  );
  let dict = dictionary! { "Type" => "Font", "Subtype" => "TrueType", "BaseFont" => "F2" };
  assert!(!load_font(&doc, &dict).weight_known);
}

#[test]
fn anonymous_font_with_descriptor_evidence_keeps_its_weight() {
  let mut doc = Document::with_version("1.7");
  let desc = doc.add_object(
    dictionary! { "Type" => "FontDescriptor", "FontName" => "CIDFont+F1", "FontWeight" => 700 },
  );
  let dict = dictionary! { "Type" => "Font", "Subtype" => "TrueType", "BaseFont" => "CIDFont+F1", "FontDescriptor" => Object::Reference(desc) };
  let f = load_font(&doc, &dict);
  assert!(f.weight_known, "FontWeight is evidence");
  assert!(f.bold);
}

#[test]
fn helvetica_bold_keeps_known_weight() {
  let doc = Document::with_version("1.7");
  let dict = dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica-Bold" };
  let f = load_font(&doc, &dict);
  assert!(f.weight_known && f.bold && !f.symbolic);
}

#[test]
fn cid_width_array_handles_both_forms() {
  let doc = Document::with_version("1.7");
  let mut out = HashMap::new();
  let w = vec![
    Object::Integer(1),
    Object::Array(vec![500.into(), 600.into()]),
    Object::Integer(10),
    Object::Integer(12),
    Object::Integer(250),
  ];
  parse_cid_widths(&doc, &w, &mut out);
  assert_eq!(out.get(&1), Some(&0.5));
  assert_eq!(out.get(&2), Some(&0.6));
  assert_eq!(out.get(&10), Some(&0.25));
  assert_eq!(out.get(&12), Some(&0.25));
  assert_eq!(out.get(&13), None);
}

#[test]
fn type0_font_is_two_byte_with_default_width() {
  let mut doc = Document::with_version("1.7");
  let desc_font = doc.add_object(dictionary! {
    "Type" => "Font", "Subtype" => "CIDFontType2", "BaseFont" => "ABCDEF+Roboto", "DW" => 700,
    "W" => vec![Object::Integer(36), Object::Array(vec![640.into()])],
  });
  let font = dictionary! {
    "Type" => "Font", "Subtype" => "Type0", "BaseFont" => "ABCDEF+Roboto", "Encoding" => "Identity-H",
    "DescendantFonts" => vec![Object::Reference(desc_font)],
  };
  let f = load_font(&doc, &font);
  assert!(f.two_byte);
  assert_eq!(f.width(36), 0.64);
  assert_eq!(f.width(37), 0.7);
}

#[test]
fn to_unicode_wins_over_a_differences_encoding_lopdf_cannot_build() {
  // Acrobat PDFMaker: custom codes 28..31 named /ygrave /lacute /pdotaccent /idotaccent over
  // WinAnsi; lopdf knows no /idotaccent and drops the whole encoding for StandardEncoding,
  // while the ToUnicode map says exactly what every code is.
  let mut doc = Document::with_version("1.7");
  let cmap = doc.add_object(Stream::new(
      dictionary! {},
      b"1 begincodespacerange <00> <FF> endcodespacerange\n6 beginbfchar <1C> <1EF3> <1D> <013A> <1E> <1E57> <1F> <0069> <44> <0044> <9A> <0161> endbfchar\n".to_vec(),
    ));
  let font = dictionary! {
    "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "NDVFQS+SFProDisplay-Bold",
    "Encoding" => dictionary! {
      "BaseEncoding" => "WinAnsiEncoding",
      "Differences" => vec![Object::Integer(28), "ygrave".into(), "lacute".into(), "pdotaccent".into(), "idotaccent".into()]
    },
    "ToUnicode" => Object::Reference(cmap)
  };
  let f = load_font(&doc, &font);
  assert_eq!(f.decode(b"\x1f"), "i");
  assert_eq!(f.decode(b"\x1c"), "\u{1EF3}");
  assert_eq!(
    f.decode(b"\x9a"),
    "\u{0161}",
    "WinAnsi's upper range survives through the map"
  );
  assert_eq!(f.decode(b"D"), "D");
  assert_eq!(
    f.decode(b"a"),
    "a",
    "a code the map omits still decodes through the encoding"
  );
}

#[test]
fn ultra_fett_and_semibold_names_are_bold_but_ultralight_is_not() {
  let doc = Document::with_version("1.7");
  let bold = |base: &str| {
    load_font(
      &doc,
      &dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => base },
    )
    .bold
  };
  assert!(bold("USNFKE+GothamCond-Ultra"));
  assert!(bold("Sohne-Halbfett"), "Halbfett is 600");
  assert!(bold("Sohne-Fett"));
  assert!(bold("NotoSansJP-SemiBold"));
  assert!(bold("Lato-DemiBold"));
  assert!(!bold("Gotham-UltraLight"));
  assert!(!bold("Gotham-Ultra-Thin"));
  assert!(!bold("NotoSerifKR-Medium"));
  assert!(!bold("Sohne-Kraftig"), "500 is not bold");
}
