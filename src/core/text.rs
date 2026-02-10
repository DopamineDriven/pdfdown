use crate::types::{PageText, StructuredPageText};
use lopdf::Document;
use napi::Result;
use rayon::prelude::*;

pub(crate) fn extract_text(doc: &Document) -> Result<Vec<PageText>> {
  let pages = doc.get_pages();
  let page_count_str = pages.len().to_string();
  let page_nums: Vec<u32> = pages.keys().copied().collect();
  let mut results: Vec<PageText> = page_nums
    .par_iter()
    .map(|&page_num| {
      let raw = doc.extract_text(&[page_num]).unwrap_or_default();
      let text = strip_footer_artifacts(&raw, &page_count_str);
      PageText {
        page: page_num,
        text,
      }
    })
    .collect();
  results.sort_unstable_by_key(|p| p.page);
  Ok(results)
}

/// Normalize a line for header/footer comparison: trim whitespace and replace
/// contiguous digit sequences with `<NUM>` so "Page 1" matches "Page 42".
fn normalize_header_footer_line(line: &str) -> String {
  let trimmed = line.trim();
  let mut out = String::with_capacity(trimmed.len());
  let mut in_digits = false;
  for ch in trimmed.chars() {
    if ch.is_ascii_digit() {
      if !in_digits {
        out.push_str("<NUM>");
        in_digits = true;
      }
    } else {
      in_digits = false;
      out.push(ch);
    }
  }
  out
}

/// Strip Chromium footer artifacts from extracted text.
///
/// Chromium's Skia PDF renderer writes page footers (e.g., `1 / 38`) as 2-3
/// separate text operations. `lopdf::extract_text` concatenates these in
/// content-stream order, causing orphaned fragments like ` / \n38\n` to appear
/// mid-text on every page. This function removes the known pattern: a line
/// containing just `/` followed by a line containing just the total page count.
pub(crate) fn strip_footer_artifacts(text: &str, page_count_str: &str) -> String {
  let lines: Vec<&str> = text.lines().collect();
  if lines.len() < 2 {
    return text.to_string();
  }
  let mut skip = vec![false; lines.len()];
  for i in 0..lines.len() - 1 {
    if lines[i].trim() == "/" && lines[i + 1].trim() == page_count_str {
      skip[i] = true;
      skip[i + 1] = true;
    }
  }
  if !skip.iter().any(|&s| s) {
    return text.to_string();
  }
  lines
    .iter()
    .zip(skip.iter())
    .filter(|&(_, &s)| !s)
    .map(|(&line, _)| line)
    .collect::<Vec<_>>()
    .join("\n")
}

/// Detect repeated header/footer lines across pages and split each page's text
/// into header, body, and footer sections.
pub(crate) fn detect_headers_footers(pages: &[PageText]) -> Vec<StructuredPageText> {
  // For fewer than 3 pages, no meaningful detection — return everything as body
  if pages.len() < 3 {
    return pages
      .iter()
      .map(|p| StructuredPageText {
        page: p.page,
        header: String::new(),
        body: p.text.clone(),
        footer: String::new(),
      })
      .collect();
  }

  let threshold = (pages.len() as f64 * 0.6).ceil() as usize;
  let max_check = 3usize; // check up to 3 lines from top/bottom

  // Split each page into lines
  let page_lines: Vec<Vec<&str>> = pages.iter().map(|p| p.text.lines().collect()).collect();

  // Detect header line count: for each position 0..max_check, check if the
  // normalized line at that position appears on >= threshold pages
  let mut header_count = 0usize;
  for pos in 0..max_check {
    let mut freq = std::collections::HashMap::<String, usize>::new();
    for lines in &page_lines {
      if let Some(&line) = lines.get(pos) {
        let norm = normalize_header_footer_line(line);
        if !norm.is_empty() {
          *freq.entry(norm).or_insert(0) += 1;
        }
      }
    }
    if freq.values().any(|&c| c >= threshold) {
      header_count = pos + 1;
    } else {
      break;
    }
  }

  // Detect footer line count (from the bottom)
  let mut footer_count = 0usize;
  for pos in 0..max_check {
    let mut freq = std::collections::HashMap::<String, usize>::new();
    for lines in &page_lines {
      if lines.len() > pos {
        let idx = lines.len() - 1 - pos;
        // Don't overlap with headers
        if idx >= header_count {
          let norm = normalize_header_footer_line(lines[idx]);
          if !norm.is_empty() {
            *freq.entry(norm).or_insert(0) += 1;
          }
        }
      }
    }
    if freq.values().any(|&c| c >= threshold) {
      footer_count = pos + 1;
    } else {
      break;
    }
  }

  pages
    .iter()
    .zip(page_lines.iter())
    .map(|(p, lines)| {
      let total = lines.len();
      let h_end = header_count.min(total);
      let f_start = if footer_count > 0 {
        total.saturating_sub(footer_count).max(h_end)
      } else {
        total
      };

      let header = lines[..h_end].join("\n");
      let body = lines[h_end..f_start].join("\n");
      let footer = lines[f_start..].join("\n");

      StructuredPageText {
        page: p.page,
        header,
        body,
        footer,
      }
    })
    .collect()
}

pub(crate) fn extract_structured_text(doc: &Document) -> Result<Vec<StructuredPageText>> {
  let pages = extract_text(doc)?;
  Ok(detect_headers_footers(&pages))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn strip_basic_footer_artifact() {
    let text = "Some content\n/\n38\nMore content";
    let result = strip_footer_artifacts(text, "38");
    assert_eq!(result, "Some content\nMore content");
  }

  #[test]
  fn strip_footer_artifact_with_whitespace() {
    let text = "Some content\n  /  \n  38  \nMore content";
    let result = strip_footer_artifacts(text, "38");
    assert_eq!(result, "Some content\nMore content");
  }

  #[test]
  fn no_match_passthrough() {
    let text = "Some content\nNo footer here\nMore content";
    let result = strip_footer_artifacts(text, "38");
    assert_eq!(result, text);
  }

  #[test]
  fn multiple_occurrences() {
    let text = "Page one\n/\n38\nPage two\n/\n38\nPage three";
    let result = strip_footer_artifacts(text, "38");
    assert_eq!(result, "Page one\nPage two\nPage three");
  }

  #[test]
  fn at_start_of_text() {
    let text = "/\n38\nContent after";
    let result = strip_footer_artifacts(text, "38");
    assert_eq!(result, "Content after");
  }

  #[test]
  fn at_end_of_text() {
    let text = "Content before\n/\n38";
    let result = strip_footer_artifacts(text, "38");
    assert_eq!(result, "Content before");
  }

  #[test]
  fn empty_input() {
    let result = strip_footer_artifacts("", "38");
    assert_eq!(result, "");
  }

  #[test]
  fn single_line_input() {
    let result = strip_footer_artifacts("just one line", "38");
    assert_eq!(result, "just one line");
  }

  #[test]
  fn consecutive_pairs() {
    // Two pairs back to back: `/\n38\n/\n38`
    let text = "start\n/\n38\n/\n38\nend";
    let result = strip_footer_artifacts(text, "38");
    assert_eq!(result, "start\nend");
  }

  #[test]
  fn slash_not_followed_by_count() {
    let text = "Some content\n/\n99\nMore content";
    let result = strip_footer_artifacts(text, "38");
    assert_eq!(result, text);
  }

  #[test]
  fn slash_with_extra_text_not_stripped() {
    let text = "Some content\n/ extra\n38\nMore content";
    let result = strip_footer_artifacts(text, "38");
    assert_eq!(result, text);
  }

  #[test]
  fn count_with_extra_text_not_stripped() {
    let text = "Some content\n/\n38 pages\nMore content";
    let result = strip_footer_artifacts(text, "38");
    assert_eq!(result, text);
  }
}
