//! Key-value geometry: where a label's value sits. For each lexicon hit the candidates are the
//! rest of the label's own segment, the nearest aligned segment below, and the next segment to
//! the right on the same baseline; each carries the scores the field logic combines.

use super::lexicon::{Field, Hit, Label, find_labels, has_terms_at_start, is_title_row};
use super::segments::Segment;

/// Lines below a label that a value may sit on, as multiples of the label's line height.
const BELOW_LINES: f64 = 3.0;
const LINE_EM: f64 = 1.2;
/// A value below is aligned with its label within this many ems of the label's left edge.
const ALIGN_EM: f64 = 1.5;

/// A value never runs longer than this many tokens; a longer remainder is prose.
pub(crate) const MAX_VALUE_TOKENS: usize = 12;

#[derive(Debug, Clone)]
pub(crate) struct Candidate {
  pub text: String,
  /// Indexes into the page's words.
  pub words: Vec<usize>,
  /// The page's position in the array it came from.
  pub page_index: usize,
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
    page_index: s.page_index,
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
      t.page_index == s.page_index
        && t.y1 <= s.y0 + 1e-6
        && s.y0 - t.y1 <= reach
        && (t.x0 - s.x0).abs() <= ALIGN_EM * s.font_size
        && !is_labelled(&hits[i])
    })
    .max_by(|&a, &b| segments[a].y1.total_cmp(&segments[b].y1))
}

/// Every unlabelled segment to the right of `s` on its baseline, nearest first.
fn right_all(segments: &[Segment], hits: &[Vec<Hit>], s: &Segment) -> Vec<usize> {
  let mut out: Vec<usize> = (0..segments.len())
    .filter(|&i| {
      let t = &segments[i];
      t.page_index == s.page_index && t.line == s.line && t.x0 > s.x1 && !is_labelled(&hits[i])
    })
    .collect();
  out.sort_by(|&a, &b| segments[a].x0.total_cmp(&segments[b].x0));
  out
}

/// Whether the column holding `t` is headed by a bare `Total` label: walking up from `t`, the
/// nearest segment that overlaps it horizontally and carries no digit is the column's header.
fn under_total_header(segments: &[Segment], t: &Segment) -> bool {
  let header = segments
    .iter()
    .filter(|u| {
      u.page_index == t.page_index
        && u.y0 >= t.y1
        && u.x0 < t.x1
        && t.x0 < u.x1
        && !u.text.chars().any(|c| c.is_ascii_digit())
    })
    .min_by(|a, b| a.y0.total_cmp(&b.y0));
  let Some(h) = header else {
    return false;
  };
  let words: Vec<&str> = h
    .tokens
    .iter()
    .map(String::as_str)
    .filter(|w| w.chars().any(char::is_alphanumeric))
    .collect();
  find_labels(&words, Some(Field::Total))
    .iter()
    .any(|hit| hit.start == 0 && hit.score >= 0.8 && hit.remainder.is_empty())
}

/// Whether a field's value is an amount: its label may head a row of column amounts.
fn is_money(field: Field) -> bool {
  matches!(
    field,
    Field::SubTotal | Field::Tax | Field::Total | Field::AmountDue
  )
}

fn neighbour(t: &Segment, hit: &Hit, geometry: f64) -> Candidate {
  Candidate {
    text: t.text.clone(),
    words: t.words.clone(),
    page_index: t.page_index,
    label: hit.label,
    label_score: hit.score,
    geometry_score: geometry,
    ocr_confidence: t.ocr_confidence,
  }
}

/// Every candidate value for `field` across the segments, from the shared per-segment hits,
/// unscored by any validator yet: the rest of the label's segment, the aligned unlabelled line
/// below, and the unlabelled segment to the right on the same baseline. A totals label takes
/// every unlabelled segment to its right (`Sub Total: | 1,200.00 | 300.00 | 1,500.00`): the one
/// under a `Total` column header scores highest, then the last column, where an invoice keeps
/// its total.
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
      // A title row (`INVOICE`) heads the page: what sits below it is the body, not its value.
      if !is_title_row(hit.label)
        && let Some(i) = below(segments, hits, s)
      {
        out.push(neighbour(&segments[i], hit, 0.8));
      }
      let to_right = right_all(segments, hits, s);
      if is_money(field) {
        let last = to_right.len().saturating_sub(1);
        for (n, &i) in to_right.iter().enumerate() {
          let t = &segments[i];
          let geometry = if under_total_header(segments, t) {
            0.8
          } else if n == last {
            0.7
          } else {
            0.6
          };
          out.push(neighbour(t, hit, geometry));
        }
      } else if let Some(&i) = to_right.first() {
        out.push(neighbour(&segments[i], hit, 0.7));
      }
    }
  }
  out
}

#[cfg(test)]
mod tests;
