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
