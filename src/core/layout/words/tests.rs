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
  let (words, lines) = words_to_lines(glyphs_to_words(&g), &fonts, &[]);
  assert_eq!(lines.len(), 2);
  assert_eq!(lines[0].text, "left right");
  assert_eq!(lines[1].text, "low");
  assert_eq!((lines[0].first_word, lines[0].word_count), (0, 2));
  assert_eq!((lines[1].first_word, lines[1].word_count), (2, 1));
  assert_eq!(words.iter().map(|w| w.line).collect::<Vec<_>>(), [0, 0, 1]);
  assert_eq!(words[0].font_name.as_deref(), Some("Helvetica"));
  assert_eq!(words[0].bold, Some(false));
}
