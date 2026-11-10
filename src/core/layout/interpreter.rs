//! A content-stream interpreter that tracks just enough state to place text:
//! the graphics state stack (`q`/`Q`, `cm`), the text object (`BT`/`ET`, `Tm`,
//! `Td`, `TD`, `T*`, `TL`), text state (`Tf`, `Tc`, `Tw`, `Tz`, `Ts`, `Tr`),
//! the show operators (`Tj`, `TJ`, `'`, `"`) and `Do` for form and image
//! XObjects (ISO 32000-1 §8.4, §9.3, §9.4).

use super::fonts::{FontInfo, load_font};
use crate::types::PlacedImage;
use lopdf::content::Content;
use lopdf::{Dictionary, Document, Object, ObjectId};
use std::collections::HashMap;

/// Nested form XObjects deeper than this are not entered.
const MAX_FORM_DEPTH: usize = 12;
/// A `TJ` adjustment at least this large (in 1/1000 em, moving the pen
/// backwards) is treated as a word gap even when no space glyph is shown.
const TJ_WORD_GAP: f64 = 200.0;

/// Affine matrix `[a b c d e f]` in PDF order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Matrix {
  pub a: f64,
  pub b: f64,
  pub c: f64,
  pub d: f64,
  pub e: f64,
  pub f: f64,
}

impl Matrix {
  pub const IDENTITY: Matrix = Matrix {
    a: 1.0,
    b: 0.0,
    c: 0.0,
    d: 1.0,
    e: 0.0,
    f: 0.0,
  };

  pub fn translate(tx: f64, ty: f64) -> Matrix {
    Matrix {
      e: tx,
      f: ty,
      ..Matrix::IDENTITY
    }
  }

  /// `self × other`: apply `self` first, then `other` (PDF's convention for `cm`).
  pub fn mul(self, o: Matrix) -> Matrix {
    Matrix {
      a: self.a * o.a + self.b * o.c,
      b: self.a * o.b + self.b * o.d,
      c: self.c * o.a + self.d * o.c,
      d: self.c * o.b + self.d * o.d,
      e: self.e * o.a + self.f * o.c + o.e,
      f: self.e * o.b + self.f * o.d + o.f,
    }
  }

  pub fn apply(self, x: f64, y: f64) -> (f64, f64) {
    (
      self.a * x + self.c * y + self.e,
      self.b * x + self.d * y + self.f,
    )
  }

  /// Length of the transformed unit vertical vector: the effective vertical scale.
  pub fn scale_y(self) -> f64 {
    (self.c * self.c + self.d * self.d).sqrt()
  }

  fn from_operands(ops: &[Object]) -> Option<Matrix> {
    if ops.len() < 6 {
      return None;
    }
    let v: Vec<f64> = ops.iter().take(6).map(number).collect::<Option<_>>()?;
    Some(Matrix {
      a: v[0],
      b: v[1],
      c: v[2],
      d: v[3],
      e: v[4],
      f: v[5],
    })
  }

  fn from_array(doc: &Document, obj: &Object) -> Option<Matrix> {
    match deref(doc, obj)? {
      Object::Array(a) => Matrix::from_operands(a),
      _ => None,
    }
  }
}

fn number(obj: &Object) -> Option<f64> {
  match obj {
    Object::Integer(i) => Some(*i as f64),
    Object::Real(r) => Some(f64::from(*r)),
    _ => None,
  }
}

fn deref<'a>(doc: &'a Document, obj: &'a Object) -> Option<&'a Object> {
  match obj {
    Object::Reference(id) => doc.get_object(*id).ok(),
    other => Some(other),
  }
}

fn bbox_of(m: Matrix, x0: f64, y0: f64, x1: f64, y1: f64) -> (f64, f64, f64, f64) {
  let pts = [
    m.apply(x0, y0),
    m.apply(x1, y0),
    m.apply(x0, y1),
    m.apply(x1, y1),
  ];
  let mut out = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
  for (x, y) in pts {
    out.0 = out.0.min(x);
    out.1 = out.1.min(y);
    out.2 = out.2.max(x);
    out.3 = out.3.max(y);
  }
  out
}

/// Font identity shared by every glyph set in that font.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FontMeta {
  pub name: String,
  pub bold: bool,
  pub italic: bool,
  pub symbolic: bool,
  pub weight_known: bool,
}

/// One positioned glyph in user space.
#[derive(Debug, Clone)]
pub(crate) struct Glyph {
  pub text: String,
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
  /// Baseline y in user space.
  pub base_y: f64,
  /// Font size after the CTM, in points.
  pub font_size: f64,
  pub font: usize,
  pub is_space: bool,
  /// A word gap preceded this glyph (`Tw` space or a large `TJ` adjustment).
  pub break_before: bool,
}

#[derive(Clone)]
struct GState {
  ctm: Matrix,
  font: Option<usize>,
  size: f64,
  char_spacing: f64,
  word_spacing: f64,
  hscale: f64,
  leading: f64,
  rise: f64,
}

impl GState {
  fn new(ctm: Matrix) -> Self {
    GState {
      ctm,
      font: None,
      size: 0.0,
      char_spacing: 0.0,
      word_spacing: 0.0,
      hscale: 1.0,
      leading: 0.0,
      rise: 0.0,
    }
  }
}

pub(crate) struct Interpreter<'a> {
  doc: &'a Document,
  fonts: Vec<FontInfo<'a>>,
  font_index: HashMap<FontKey, usize>,
  pub glyphs: Vec<Glyph>,
  pub images: Vec<PlacedImage>,
  pub warnings: Vec<String>,
  tm: Matrix,
  tlm: Matrix,
  pending_break: bool,
}

#[derive(Hash, PartialEq, Eq)]
enum FontKey {
  Object(ObjectId),
  /// Inline font dictionaries have no identity; keyed by resource name and depth.
  Inline(Vec<u8>, usize),
}

impl<'a> Interpreter<'a> {
  pub fn new(doc: &'a Document) -> Self {
    Interpreter {
      doc,
      fonts: Vec::new(),
      font_index: HashMap::new(),
      glyphs: Vec::new(),
      images: Vec::new(),
      warnings: Vec::new(),
      tm: Matrix::IDENTITY,
      tlm: Matrix::IDENTITY,
      pending_break: false,
    }
  }

  pub fn font_metas(&self) -> Vec<FontMeta> {
    self
      .fonts
      .iter()
      .map(|f| FontMeta {
        name: f.name.clone(),
        bold: f.bold,
        italic: f.italic,
        symbolic: f.symbolic,
        weight_known: f.weight_known,
      })
      .collect()
  }

  /// Run a content stream with the given resources and initial CTM.
  pub fn run(&mut self, content: &[u8], resources: &'a Dictionary, ctm: Matrix) {
    self.run_inner(content, resources, ctm, 0);
  }

  fn font_slot(&mut self, resources: &'a Dictionary, name: &[u8], depth: usize) -> Option<usize> {
    let fonts = match deref(self.doc, resources.get(b"Font").ok()?)? {
      Object::Dictionary(d) => d,
      _ => return None,
    };
    let entry = fonts.get(name).ok()?;
    let key = match entry {
      Object::Reference(id) => FontKey::Object(*id),
      _ => FontKey::Inline(name.to_vec(), depth),
    };
    if let Some(&i) = self.font_index.get(&key) {
      return Some(i);
    }
    let dict = match deref(self.doc, entry)? {
      Object::Dictionary(d) => d,
      _ => return None,
    };
    let info = load_font(self.doc, dict);
    self.fonts.push(info);
    let i = self.fonts.len() - 1;
    self.font_index.insert(key, i);
    Some(i)
  }

  fn run_inner(&mut self, content: &[u8], resources: &'a Dictionary, ctm: Matrix, depth: usize) {
    let ops = match Content::decode(content) {
      Ok(c) => c.operations,
      Err(e) => {
        self
          .warnings
          .push(format!("content stream could not be parsed: {e}"));
        return;
      }
    };
    let mut gs = GState::new(ctm);
    let mut stack: Vec<GState> = Vec::new();

    for op in &ops {
      let o = &op.operands;
      match op.operator.as_str() {
        "q" => stack.push(gs.clone()),
        "Q" => {
          if let Some(s) = stack.pop() {
            gs = s;
          }
        }
        "cm" => {
          if let Some(m) = Matrix::from_operands(o) {
            gs.ctm = m.mul(gs.ctm);
          }
        }
        "BT" => {
          self.tm = Matrix::IDENTITY;
          self.tlm = Matrix::IDENTITY;
          self.pending_break = true;
        }
        "ET" => self.pending_break = true,
        "Tf" => {
          if let (Some(Object::Name(n)), Some(size)) = (o.first(), o.get(1).and_then(number)) {
            gs.font = self.font_slot(resources, n, depth);
            gs.size = size;
          }
        }
        "Td" => {
          if let (Some(tx), Some(ty)) = (o.first().and_then(number), o.get(1).and_then(number)) {
            self.tlm = Matrix::translate(tx, ty).mul(self.tlm);
            self.tm = self.tlm;
          }
        }
        "TD" => {
          if let (Some(tx), Some(ty)) = (o.first().and_then(number), o.get(1).and_then(number)) {
            gs.leading = -ty;
            self.tlm = Matrix::translate(tx, ty).mul(self.tlm);
            self.tm = self.tlm;
          }
        }
        "Tm" => {
          if let Some(m) = Matrix::from_operands(o) {
            self.tlm = m;
            self.tm = m;
          }
        }
        "T*" => self.next_line(&gs),
        "TL" => gs.leading = o.first().and_then(number).unwrap_or(gs.leading),
        "Tc" => gs.char_spacing = o.first().and_then(number).unwrap_or(gs.char_spacing),
        "Tw" => gs.word_spacing = o.first().and_then(number).unwrap_or(gs.word_spacing),
        "Tz" => {
          gs.hscale = o
            .first()
            .and_then(number)
            .map(|v| v / 100.0)
            .unwrap_or(gs.hscale)
        }
        "Ts" => gs.rise = o.first().and_then(number).unwrap_or(gs.rise),
        "Tj" => {
          if let Some(Object::String(bytes, _)) = o.first() {
            self.show(bytes, &gs);
          }
        }
        "'" => {
          self.next_line(&gs);
          if let Some(Object::String(bytes, _)) = o.first() {
            self.show(bytes, &gs);
          }
        }
        "\"" => {
          if let (Some(aw), Some(ac)) = (o.first().and_then(number), o.get(1).and_then(number)) {
            gs.word_spacing = aw;
            gs.char_spacing = ac;
          }
          self.next_line(&gs);
          if let Some(Object::String(bytes, _)) = o.get(2) {
            self.show(bytes, &gs);
          }
        }
        "TJ" => {
          if let Some(Object::Array(items)) = o.first() {
            for item in items {
              match item {
                Object::String(bytes, _) => self.show(bytes, &gs),
                other => {
                  if let Some(adj) = number(other) {
                    let tx = -adj / 1000.0 * gs.size * gs.hscale;
                    self.tm = Matrix::translate(tx, 0.0).mul(self.tm);
                    if adj <= -TJ_WORD_GAP {
                      self.pending_break = true;
                    }
                  }
                }
              }
            }
          }
        }
        "Do" => {
          if let Some(Object::Name(n)) = o.first() {
            self.do_xobject(n, resources, &gs, depth);
          }
        }
        _ => {}
      }
    }
  }

  fn next_line(&mut self, gs: &GState) {
    self.tlm = Matrix::translate(0.0, -gs.leading).mul(self.tlm);
    self.tm = self.tlm;
    self.pending_break = true;
  }

  fn show(&mut self, bytes: &[u8], gs: &GState) {
    let Some(fi) = gs.font else {
      return;
    };
    let font = &self.fonts[fi];
    let step = if font.two_byte { 2 } else { 1 };
    let mut i = 0;
    while i < bytes.len() {
      let end = (i + step).min(bytes.len());
      let chunk = &bytes[i..end];
      i = end;
      let code = chunk.iter().fold(0u32, |acc, b| acc << 8 | u32::from(*b));
      let w0 = font.width(code);
      let text = font.decode(chunk);
      let is_space = !font.two_byte && code == 32 || text.trim().is_empty();

      let trm = Matrix {
        a: gs.size * gs.hscale,
        b: 0.0,
        c: 0.0,
        d: gs.size,
        e: 0.0,
        f: gs.rise,
      }
      .mul(self.tm)
      .mul(gs.ctm);
      let (x0, y0, x1, y1) = bbox_of(trm, 0.0, font.descent, w0, font.ascent);
      let (_, base_y) = trm.apply(0.0, 0.0);
      let font_size = gs.size * self.tm.mul(gs.ctm).scale_y();

      if !text.is_empty() {
        self.glyphs.push(Glyph {
          text,
          x0,
          y0,
          x1,
          y1,
          base_y,
          font_size,
          font: fi,
          is_space,
          break_before: self.pending_break,
        });
        self.pending_break = false;
      }

      let mut tx = w0 * gs.size + gs.char_spacing;
      if !font.two_byte && code == 32 {
        tx += gs.word_spacing;
        if gs.word_spacing > 0.0 {
          self.pending_break = true;
        }
      }
      self.tm = Matrix::translate(tx * gs.hscale, 0.0).mul(self.tm);
    }
  }

  fn do_xobject(&mut self, name: &[u8], resources: &'a Dictionary, gs: &GState, depth: usize) {
    let Some(xobjects) = resources
      .get(b"XObject")
      .ok()
      .and_then(|o| deref(self.doc, o))
      .and_then(|o| match o {
        Object::Dictionary(d) => Some(d),
        _ => None,
      })
    else {
      return;
    };
    let Some(Object::Stream(stream)) = xobjects.get(name).ok().and_then(|o| deref(self.doc, o))
    else {
      return;
    };
    let subtype = stream
      .dict
      .get(b"Subtype")
      .ok()
      .and_then(|o| o.as_name().ok())
      .unwrap_or(b"");
    match subtype {
      b"Image" => {
        let (x0, y0, x1, y1) = bbox_of(gs.ctm, 0.0, 0.0, 1.0, 1.0);
        self.images.push(PlacedImage {
          xobject_name: String::from_utf8_lossy(name).to_string(),
          x0,
          y0,
          x1,
          y1,
        });
      }
      b"Form" => {
        if depth >= MAX_FORM_DEPTH {
          self.warnings.push(format!(
            "form XObject {} nested deeper than {MAX_FORM_DEPTH}; skipped",
            String::from_utf8_lossy(name)
          ));
          return;
        }
        let matrix = stream
          .dict
          .get(b"Matrix")
          .ok()
          .and_then(|o| Matrix::from_array(self.doc, o))
          .unwrap_or(Matrix::IDENTITY);
        let form_resources: &'a Dictionary = stream
          .dict
          .get(b"Resources")
          .ok()
          .and_then(|o| deref(self.doc, o))
          .and_then(|o| match o {
            Object::Dictionary(d) => Some(d),
            _ => None,
          })
          .unwrap_or(resources);
        let Ok(content) = stream.decompressed_content() else {
          return;
        };
        let saved_tm = self.tm;
        let saved_tlm = self.tlm;
        self.run_inner(&content, form_resources, matrix.mul(gs.ctm), depth + 1);
        self.tm = saved_tm;
        self.tlm = saved_tlm;
      }
      _ => {}
    }
  }
}

#[cfg(test)]
mod tests;
