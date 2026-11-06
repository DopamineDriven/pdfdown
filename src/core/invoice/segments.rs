//! Lines split into segments at wide horizontal gaps, so a label and the value column beside
//! it, or two columns on one baseline, are read apart. Everything downstream works on segments.

use super::lexicon::{Hit, find_labels};
use crate::types::{FieldSpan, PageLayout};

/// Gap, in multiples of the line's font size, that separates two segments on one baseline.
const GAP_EM: f64 = 2.5;

#[derive(Debug, Clone)]
pub(crate) struct Segment {
  pub page: u32,
  pub line: u32,
  /// Indexes into the page's `words`, left to right.
  pub words: Vec<usize>,
  pub tokens: Vec<String>,
  pub text: String,
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
  pub font_size: f64,
  pub bold: bool,
  /// Mean confidence of the OCR words; `None` for native words.
  pub ocr_confidence: Option<f64>,
}

fn median(values: &mut [f64]) -> f64 {
  if values.is_empty() {
    return 0.0;
  }
  values.sort_by(f64::total_cmp);
  values[values.len() / 2]
}

fn word_size(page: &PageLayout, i: usize) -> f64 {
  let w = &page.words[i];
  w.font_size
    .filter(|s| *s > 0.0)
    .unwrap_or((w.y1 - w.y0).max(1.0))
}

fn make(page: &PageLayout, line: u32, words: Vec<usize>) -> Segment {
  let mut sizes: Vec<f64> = words.iter().map(|&i| word_size(page, i)).collect();
  let bold = words
    .iter()
    .filter(|&&i| page.words[i].bold == Some(true))
    .count()
    * 2
    > words.len();
  let confidences: Vec<f64> = words
    .iter()
    .filter_map(|&i| page.words[i].confidence)
    .collect();
  let tokens: Vec<String> = words.iter().map(|&i| page.words[i].text.clone()).collect();
  let span = span_of(page, &words);
  Segment {
    page: page.page,
    line,
    text: tokens.join(" "),
    tokens,
    x0: span.x0,
    y0: span.y0,
    x1: span.x1,
    y1: span.y1,
    font_size: median(&mut sizes),
    bold,
    ocr_confidence: (!confidences.is_empty())
      .then(|| confidences.iter().sum::<f64>() / confidences.len() as f64),
    words,
  }
}

/// The page's lines, each split where a horizontal gap reaches `GAP_EM` of the line's font size.
pub(crate) fn segments(page: &PageLayout) -> Vec<Segment> {
  let mut out = Vec::new();
  for (li, line) in page.lines.iter().enumerate() {
    let first = line.first_word as usize;
    let count = line.word_count as usize;
    if count == 0 || first + count > page.words.len() {
      continue;
    }
    let mut sizes: Vec<f64> = (first..first + count).map(|i| word_size(page, i)).collect();
    let gap = GAP_EM * median(&mut sizes);
    let mut current: Vec<usize> = vec![first];
    for i in first + 1..first + count {
      let prev = &page.words[i - 1];
      if page.words[i].x0 - prev.x1 >= gap {
        out.push(make(page, li as u32, std::mem::take(&mut current)));
      }
      current.push(i);
    }
    if !current.is_empty() {
      out.push(make(page, li as u32, current));
    }
  }
  out
}

/// Every label hit of every field, per segment, computed once per document and shared by the
/// anchors, parties and priors.
pub(crate) fn label_hits(segments: &[Segment]) -> Vec<Vec<Hit>> {
  segments
    .iter()
    .map(|s| {
      let tokens: Vec<&str> = s.tokens.iter().map(String::as_str).collect();
      find_labels(&tokens, None)
    })
    .collect()
}

/// The box around the given words of a page.
pub(crate) fn span_of(page: &PageLayout, words: &[usize]) -> FieldSpan {
  let mut span = FieldSpan {
    page: page.page,
    x0: f64::MAX,
    y0: f64::MAX,
    x1: f64::MIN,
    y1: f64::MIN,
  };
  for &i in words {
    let w = &page.words[i];
    span.x0 = span.x0.min(w.x0);
    span.y0 = span.y0.min(w.y0);
    span.x1 = span.x1.max(w.x1);
    span.y1 = span.y1.max(w.y1);
  }
  if words.is_empty() {
    span.x0 = 0.0;
    span.y0 = 0.0;
    span.x1 = 0.0;
    span.y1 = 0.0;
  }
  span
}

#[cfg(test)]
mod tests {
  use super::super::testing::page;
  use super::*;

  #[test]
  fn a_line_splits_into_segments_at_a_wide_gap() {
    let p = page(&[
      (43.0, 700.0, 10.0, false, "From:"),
      (348.0, 700.0, 10.0, false, "Invoice Number INV-3337"),
    ]);
    let segs = segments(&p);
    assert_eq!(
      segs.iter().map(|s| s.text.as_str()).collect::<Vec<_>>(),
      ["From:", "Invoice Number INV-3337"]
    );
    assert_eq!(segs[1].words.len(), 3);
    assert_eq!(segs[1].font_size, 10.0);
    assert!(!segs[1].bold);
    assert_eq!(segs[1].ocr_confidence, None);
    let span = span_of(&p, &segs[1].words);
    assert_eq!(span.page, 1);
    assert!((span.x0 - 348.0).abs() < 1e-9 && span.y0 == 700.0 && span.y1 == 710.0);
  }
}
