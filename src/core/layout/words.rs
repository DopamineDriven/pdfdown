//! Glyphs → words → lines. Purely geometric: works for native glyph boxes and
//! for OCR word boxes alike.

use super::interpreter::{FontMeta, Glyph};
use crate::types::{TextLine, TextWord};

/// A gap wider than this fraction of the font size separates words.
const WORD_GAP_EM: f64 = 0.25;
/// A pen moving backwards by more than this fraction of the font size starts a new word.
const BACKTRACK_EM: f64 = 0.5;

/// Intermediate word used by both the native and OCR paths.
#[derive(Debug, Clone)]
pub(crate) struct Word {
  pub text: String,
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
  pub baseline: f64,
  pub font_size: Option<f64>,
  pub font: Option<usize>,
  pub confidence: Option<f64>,
  /// Spreadsheet words: index into the page's cell references.
  pub cell: Option<usize>,
}

impl Word {
  fn from_glyph(g: &Glyph) -> Word {
    Word {
      text: g.text.clone(),
      x0: g.x0,
      y0: g.y0,
      x1: g.x1,
      y1: g.y1,
      baseline: g.base_y,
      font_size: Some(g.font_size),
      font: Some(g.font),
      confidence: None,
      cell: None,
    }
  }

  fn extend(&mut self, g: &Glyph) {
    self.text.push_str(&g.text);
    self.x0 = self.x0.min(g.x0);
    self.y0 = self.y0.min(g.y0);
    self.x1 = self.x1.max(g.x1);
    self.y1 = self.y1.max(g.y1);
    if let Some(fs) = self.font_size {
      self.font_size = Some(fs.max(g.font_size));
    }
  }
}

/// Group consecutive glyphs into words: split at space glyphs, explicit word
/// gaps, baseline changes, backwards pen moves, and horizontal gaps.
pub(crate) fn glyphs_to_words(glyphs: &[Glyph]) -> Vec<Word> {
  let mut words: Vec<Word> = Vec::new();
  let mut current: Option<Word> = None;
  for g in glyphs {
    if g.is_space {
      if let Some(w) = current.take() {
        words.push(w);
      }
      continue;
    }
    let starts_new = match &current {
      None => true,
      Some(w) => {
        let em = g.font_size.max(1e-6);
        g.break_before
          || (g.base_y - w.baseline).abs() > 0.5 * em
          || g.x0 - w.x1 > WORD_GAP_EM * em
          || g.x0 < w.x1 - BACKTRACK_EM * em
      }
    };
    if starts_new {
      if let Some(w) = current.take() {
        words.push(w);
      }
      current = Some(Word::from_glyph(g));
    } else if let Some(w) = current.as_mut() {
      w.extend(g);
    }
  }
  if let Some(w) = current {
    words.push(w);
  }
  words.retain(|w| !w.text.trim().is_empty());
  words
}

struct LineAcc {
  words: Vec<Word>,
  baseline_sum: f64,
  y0: f64,
  y1: f64,
}

impl LineAcc {
  fn baseline(&self) -> f64 {
    self.baseline_sum / self.words.len() as f64
  }
}

fn vertical_overlap(a0: f64, a1: f64, b0: f64, b1: f64) -> f64 {
  let inter = (a1.min(b1) - a0.max(b0)).max(0.0);
  let shortest = (a1 - a0).min(b1 - b0).max(1e-6);
  inter / shortest
}

/// Cluster words into baseline bands, order bands top to bottom and words
/// left to right, and emit the public `TextWord` / `TextLine` arrays. `cells`
/// resolves a spreadsheet word's cell index to its reference; PDF callers pass
/// an empty slice.
pub(crate) fn words_to_lines(
  words: Vec<Word>,
  fonts: &[FontMeta],
  cells: &[String],
) -> (Vec<TextWord>, Vec<TextLine>) {
  if words.is_empty() {
    return (Vec::new(), Vec::new());
  }
  let mut heights: Vec<f64> = words.iter().map(|w| w.y1 - w.y0).collect();
  heights.sort_by(f64::total_cmp);
  let median_height = heights[heights.len() / 2].max(1e-6);
  let tolerance = 0.5 * median_height;

  let mut sorted = words;
  sorted.sort_by(|a, b| {
    b.baseline
      .total_cmp(&a.baseline)
      .then(a.x0.total_cmp(&b.x0))
  });

  let mut lines: Vec<LineAcc> = Vec::new();
  for w in sorted {
    let fit = lines.iter_mut().find(|l| {
      (l.baseline() - w.baseline).abs() <= tolerance
        && vertical_overlap(l.y0, l.y1, w.y0, w.y1) > 0.5
    });
    match fit {
      Some(l) => {
        l.baseline_sum += w.baseline;
        l.y0 = l.y0.min(w.y0);
        l.y1 = l.y1.max(w.y1);
        l.words.push(w);
      }
      None => lines.push(LineAcc {
        baseline_sum: w.baseline,
        y0: w.y0,
        y1: w.y1,
        words: vec![w],
      }),
    }
  }

  lines.sort_by(|a, b| {
    b.y1
      .total_cmp(&a.y1)
      .then(a.words[0].x0.total_cmp(&b.words[0].x0))
  });

  let mut out_words: Vec<TextWord> = Vec::new();
  let mut out_lines: Vec<TextLine> = Vec::new();
  for (li, mut line) in lines.into_iter().enumerate() {
    line.words.sort_by(|a, b| a.x0.total_cmp(&b.x0));
    let first_word = out_words.len() as u32;
    let x0 = line.words.iter().map(|w| w.x0).fold(f64::MAX, f64::min);
    let x1 = line.words.iter().map(|w| w.x1).fold(f64::MIN, f64::max);
    let text = line
      .words
      .iter()
      .map(|w| w.text.as_str())
      .collect::<Vec<_>>()
      .join(" ");
    let word_count = line.words.len() as u32;
    for w in line.words {
      let meta = w.font.and_then(|i| fonts.get(i));
      out_words.push(TextWord {
        text: w.text,
        x0: w.x0,
        y0: w.y0,
        x1: w.x1,
        y1: w.y1,
        line: li as u32,
        font_size: w.font_size,
        font_name: meta.map(|m| m.name.clone()),
        bold: meta.and_then(|m| m.weight_known.then_some(m.bold)),
        italic: meta.and_then(|m| m.weight_known.then_some(m.italic)),
        symbolic: meta.map(|m| m.symbolic),
        confidence: w.confidence,
        cell: w.cell.and_then(|i| cells.get(i).cloned()),
      });
    }
    out_lines.push(TextLine {
      text,
      x0,
      y0: line.y0,
      x1,
      y1: line.y1,
      first_word,
      word_count,
    });
  }
  (out_words, out_lines)
}

#[cfg(test)]
mod tests;
