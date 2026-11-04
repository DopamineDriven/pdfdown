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
/// left to right, and emit the public `TextWord` / `TextLine` arrays.
pub(crate) fn words_to_lines(
  words: Vec<Word>,
  fonts: &[FontMeta],
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
mod tests {
  use super::*;

  fn glyph(text: &str, x0: f64, base_y: f64, width: f64, size: f64) -> Glyph {
    Glyph {
      text: text.into(),
      x0,
      y0: base_y - 0.2 * size,
      x1: x0 + width,
      y1: base_y + 0.7 * size,
      base_y,
      font_size: size,
      font: 0,
      is_space: text == " ",
      break_before: false,
    }
  }

  #[test]
  fn words_split_on_spaces_gaps_and_baselines() {
    let g = vec![
      glyph("H", 10.0, 700.0, 7.0, 10.0),
      glyph("i", 17.0, 700.0, 2.0, 10.0),
      glyph(" ", 19.0, 700.0, 3.0, 10.0),
      glyph("x", 22.0, 700.0, 5.0, 10.0),
      glyph("y", 40.0, 700.0, 5.0, 10.0), // gap of 13 > 0.25 em
      glyph("z", 45.0, 680.0, 5.0, 10.0), // new baseline
    ];
    let w = glyphs_to_words(&g);
    assert_eq!(
      w.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(),
      ["Hi", "x", "y", "z"]
    );
    assert_eq!((w[0].x0, w[0].x1), (10.0, 19.0));
  }

  #[test]
  fn break_before_and_backtracking_start_new_words() {
    let mut g = vec![
      glyph("a", 10.0, 700.0, 5.0, 10.0),
      glyph("b", 15.0, 700.0, 5.0, 10.0),
    ];
    g[1].break_before = true;
    let w = glyphs_to_words(&g);
    assert_eq!(w.len(), 2);
    let g = vec![
      glyph("a", 50.0, 700.0, 5.0, 10.0),
      glyph("b", 10.0, 700.0, 5.0, 10.0),
    ];
    assert_eq!(glyphs_to_words(&g).len(), 2);
  }

  #[test]
  fn lines_cluster_by_baseline_and_order_top_down_left_right() {
    let fonts = vec![FontMeta {
      name: "Helvetica".into(),
      bold: false,
      italic: false,
      symbolic: false,
      weight_known: true,
    }];
    let g = vec![
      glyph("low", 10.0, 600.0, 20.0, 10.0),
      glyph("right", 200.0, 700.5, 30.0, 10.0),
      glyph("left", 10.0, 700.0, 25.0, 10.0),
    ];
    let (words, lines) = words_to_lines(glyphs_to_words(&g), &fonts);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].text, "left right");
    assert_eq!(lines[1].text, "low");
    assert_eq!((lines[0].first_word, lines[0].word_count), (0, 2));
    assert_eq!((lines[1].first_word, lines[1].word_count), (2, 1));
    assert_eq!(words.iter().map(|w| w.line).collect::<Vec<_>>(), [0, 0, 1]);
    assert_eq!(words[0].font_name.as_deref(), Some("Helvetica"));
    assert_eq!(words[0].bold, Some(false));
  }
}
