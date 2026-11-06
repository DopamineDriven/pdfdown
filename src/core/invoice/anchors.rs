//! Key-value geometry: where a label's value sits. For each lexicon hit the candidates are the
//! rest of the label's own segment, the nearest aligned segment below, and the next segment to
//! the right on the same baseline; each carries the scores the field logic combines.

use super::lexicon::{Field, Hit, Label, has_terms_at_start};
use super::segments::Segment;

/// Lines below a label that a value may sit on, as multiples of the label's line height.
const BELOW_LINES: f64 = 3.0;
const LINE_EM: f64 = 1.2;
/// A value below is aligned with its label within this many ems of the label's left edge.
const ALIGN_EM: f64 = 1.5;

#[derive(Debug, Clone)]
pub(crate) struct Candidate {
  pub text: String,
  /// Indexes into the page's words.
  pub words: Vec<usize>,
  pub page: u32,
  pub label: &'static Label,
  pub label_score: f64,
  pub geometry_score: f64,
  pub ocr_confidence: Option<f64>,
}

/// `0.5·label + 0.3·validator + 0.2·geometry`, scaled by the OCR confidence when the words are
/// OCR words.
pub(crate) fn score(label: f64, validator: f64, geometry: f64, ocr: Option<f64>) -> f64 {
  (0.5 * label + 0.3 * validator + 0.2 * geometry) * ocr.unwrap_or(1.0)
}

fn same_line(segments: &[Segment], hit: &Hit, s: &Segment) -> Option<Candidate> {
  if hit.remainder.is_empty() {
    return None;
  }
  // The remainder starts in the word the label ended in when the label split inside it.
  let glued = hit.end > 0 && hit.end <= s.tokens.len() && !s.tokens[hit.end - 1].is_empty() && {
    let last = &s.tokens[hit.end - 1];
    hit
      .remainder
      .starts_with(last.trim_start_matches(|c: char| !c.is_ascii_alphanumeric()))
      && hit.remainder.len() <= last.len()
  };
  let from = if glued { hit.end - 1 } else { hit.end };
  let words: Vec<usize> = s.words[from.min(s.words.len())..].to_vec();
  let _ = segments;
  Some(Candidate {
    text: hit.remainder.clone(),
    words,
    page: s.page,
    label: hit.label,
    label_score: hit.score,
    geometry_score: 1.0,
    ocr_confidence: s.ocr_confidence,
  })
}

/// A segment that opens with a label of any field is its own key-value pair, never our value.
fn is_labelled(hits: &[Hit]) -> bool {
  hits
    .iter()
    .any(|h| h.start == 0 && h.score >= 0.8 && h.label.field != Field::DocumentTitle)
}

fn below(segments: &[Segment], hits: &[Vec<Hit>], s: &Segment) -> Option<usize> {
  let reach = BELOW_LINES * LINE_EM * s.font_size;
  (0..segments.len())
    .filter(|&i| {
      let t = &segments[i];
      t.page == s.page
        && t.y1 <= s.y0 + 1e-6
        && s.y0 - t.y1 <= reach
        && (t.x0 - s.x0).abs() <= ALIGN_EM * s.font_size
        && !is_labelled(&hits[i])
    })
    .max_by(|&a, &b| segments[a].y1.total_cmp(&segments[b].y1))
}

fn right(segments: &[Segment], hits: &[Vec<Hit>], s: &Segment) -> Option<usize> {
  (0..segments.len())
    .filter(|&i| {
      let t = &segments[i];
      t.page == s.page && t.line == s.line && t.x0 > s.x1 && !is_labelled(&hits[i])
    })
    .min_by(|&a, &b| segments[a].x0.total_cmp(&segments[b].x0))
}

fn neighbour(t: &Segment, hit: &Hit, geometry: f64) -> Candidate {
  Candidate {
    text: t.text.clone(),
    words: t.words.clone(),
    page: t.page,
    label: hit.label,
    label_score: hit.score,
    geometry_score: geometry,
    ocr_confidence: t.ocr_confidence,
  }
}

/// Every candidate value for `field` across the segments, from the shared per-segment hits,
/// unscored by any validator yet: the rest of the label's segment, the aligned unlabelled line
/// below, and the unlabelled segment to the right on the same baseline.
pub(crate) fn candidates(segments: &[Segment], hits: &[Vec<Hit>], field: Field) -> Vec<Candidate> {
  let mut out = Vec::new();
  for (s, segment_hits) in segments.iter().zip(hits) {
    if matches!(field, Field::InvoiceDate | Field::DueDate) && has_terms_at_start(segment_hits) {
      continue;
    }
    for hit in segment_hits.iter().filter(|h| h.label.field == field) {
      if hit.score < 0.6 {
        continue;
      }
      if let Some(c) = same_line(segments, hit, s) {
        out.push(c);
      }
      if let Some(i) = below(segments, hits, s) {
        out.push(neighbour(&segments[i], hit, 0.8));
      }
      if let Some(i) = right(segments, hits, s) {
        out.push(neighbour(&segments[i], hit, 0.7));
      }
    }
  }
  out
}

#[cfg(test)]
mod tests {
  use super::super::lexicon::Field;
  use super::super::segments::{label_hits, segments};
  use super::super::testing::page;
  use super::*;

  #[test]
  fn same_line_right_then_below_then_next_column() {
    let segs = segments(&page(&[(
      348.0,
      723.0,
      10.0,
      false,
      "Invoice Number INV-3337",
    )]));
    let c = candidates(&segs, &label_hits(&segs), Field::InvoiceId);
    assert_eq!(c[0].text, "INV-3337");
    assert_eq!(c[0].geometry_score, 1.0);
    assert_eq!(c[0].label_score, 1.0);
    assert_eq!(c[0].page, 1);
    assert_eq!(c[0].words.len(), 1);

    let segs = segments(&page(&[
      (50.0, 690.0, 10.0, false, "Bill To:"),
      (50.0, 676.0, 10.0, false, "Skyline Charter LLC"),
    ]));
    let c = candidates(&segs, &label_hits(&segs), Field::BillTo);
    assert_eq!(c[0].text, "Skyline Charter LLC");
    assert_eq!(c[0].geometry_score, 0.8);

    let segs = segments(&page(&[(50.0, 690.0, 10.0, false, "Terms: Net 30")]));
    assert!(candidates(&segs, &label_hits(&segs), Field::InvoiceDate).is_empty());

    assert_eq!(score(1.0, 1.0, 1.0, None), 1.0);
    assert!((score(1.0, 1.0, 0.8, Some(0.5)) - 0.48).abs() < 1e-9);
  }
}
