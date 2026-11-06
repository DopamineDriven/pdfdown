//! Hand-built pages for the invoice engine's unit tests: words laid on baselines with the same
//! half-em-per-character estimate the spreadsheet projection uses, through the real clusterer.

use super::segments::{Segment, segments};
use crate::core::layout::interpreter::FontMeta;
use crate::core::layout::words::{Word, words_to_lines};
use crate::types::{PageLayout, TextSource};

/// One run of text: `(x0, baseline, font size, bold, text)`.
pub(crate) type Run<'a> = (f64, f64, f64, bool, &'a str);

/// A 612×792 native page, y up: each run's words are `0.5 em` per character wide with a half-em
/// gap, so two runs on one baseline far apart become two segments.
pub(crate) fn page(runs: &[Run<'_>]) -> PageLayout {
  let fonts = vec![
    FontMeta {
      name: "Helvetica".into(),
      bold: false,
      italic: false,
      symbolic: false,
      weight_known: true,
    },
    FontMeta {
      name: "Helvetica-Bold".into(),
      bold: true,
      italic: false,
      symbolic: false,
      weight_known: true,
    },
  ];
  let mut words = Vec::new();
  for &(x, y, size, bold, text) in runs {
    let cw = 0.5 * size;
    let mut cursor = x;
    for word in text.split_whitespace() {
      let width = word.chars().count() as f64 * cw;
      words.push(Word {
        text: word.to_string(),
        x0: cursor,
        y0: y,
        x1: cursor + width,
        y1: y + size,
        baseline: y,
        font_size: Some(size),
        font: Some(usize::from(bold)),
        confidence: None,
        cell: None,
      });
      cursor += width + cw;
    }
  }
  let (words, lines) = words_to_lines(words, &fonts, &[]);
  PageLayout {
    page: 1,
    width: 612.0,
    height: 792.0,
    rotate: 0,
    content_rotation: 0,
    source: TextSource::Native,
    sheet: None,
    words,
    lines,
    images: Vec::new(),
    warnings: Vec::new(),
  }
}

/// The segments of a single test page (page index 0).
pub(crate) fn segments_at(page: &PageLayout) -> Vec<Segment> {
  segments(page, 0)
}

/// The header of the cluster-A look-alike (invented text), as the fixture generator lays it.
pub(crate) fn acme_page() -> PageLayout {
  page(&[
    (50.0, 760.0, 14.0, true, "ACME AVIATION SERVICES"),
    (50.0, 744.0, 10.0, false, "1200 Hangar Road"),
    (50.0, 732.0, 10.0, false, "Wichita, KS 67209"),
    (450.0, 760.0, 12.0, true, "INVOICE"),
    (380.0, 730.0, 10.0, false, "Invoice No: A-10045"),
    (380.0, 716.0, 10.0, false, "Invoice Date: 03/14/2024"),
    (380.0, 702.0, 10.0, false, "Customer PO: PO-778812"),
    (380.0, 688.0, 10.0, false, "Work Order: WO-5521"),
    (50.0, 690.0, 10.0, false, "Bill To:"),
    (50.0, 676.0, 10.0, false, "Skyline Charter LLC"),
    (50.0, 664.0, 10.0, false, "88 Terminal Drive"),
    (50.0, 652.0, 10.0, false, "Denver, CO 80249"),
    (50.0, 620.0, 10.0, false, "Aircraft Registration: N417SK"),
    (50.0, 606.0, 10.0, false, "Serial Number: 20311"),
    (400.0, 300.0, 10.0, false, "Subtotal"),
    (500.0, 300.0, 10.0, false, "$12,450.00"),
    (400.0, 286.0, 10.0, false, "Sales Tax"),
    (500.0, 286.0, 10.0, false, "$871.50"),
    (400.0, 270.0, 10.0, true, "Total Amount Due"),
    (500.0, 270.0, 10.0, true, "$13,321.50"),
  ])
}
