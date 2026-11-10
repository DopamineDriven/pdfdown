use super::*;

#[test]
fn bfchar_entries_map_single_byte_codes() {
  let cmap = ToUnicode::parse(
      b"/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n1 begincodespacerange\n<00> <FF>\nendcodespacerange\n3 beginbfchar\n<1C> <1EF3>\n<1F> <0069>\n<9A> <0161>\nendbfchar\nendcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n",
    );
  assert_eq!(cmap.lookup(b"\x1c").as_deref(), Some("\u{1EF3}"));
  assert_eq!(cmap.lookup(b"\x1f").as_deref(), Some("i"));
  assert_eq!(cmap.lookup(b"\x9a").as_deref(), Some("\u{0161}"));
  assert_eq!(cmap.lookup(b"D"), None, "a code the map does not state");
}

#[test]
fn bfrange_entries_increment_or_take_an_array() {
  let cmap = ToUnicode::parse(
      b"1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n2 beginbfrange\n<0041> <0043> <0061>\n<0010> <0011> [<004600660069> <00660069>]\nendbfrange\n",
    );
  assert_eq!(cmap.lookup(b"\x00\x41").as_deref(), Some("a"));
  assert_eq!(cmap.lookup(b"\x00\x43").as_deref(), Some("c"));
  assert_eq!(cmap.lookup(b"\x00\x44"), None);
  assert_eq!(cmap.lookup(b"\x00\x10").as_deref(), Some("Ffi"));
  assert_eq!(cmap.lookup(b"\x00\x11").as_deref(), Some("fi"));
  assert_eq!(
    cmap.lookup(b"\x41"),
    None,
    "a one-byte chunk is not a two-byte code"
  );
}

#[test]
fn destinations_may_be_surrogate_pairs_or_several_code_units() {
  let cmap = ToUnicode::parse(b"2 beginbfchar\n<05> <D83DDE00>\n<06> <006600660069>\nendbfchar\n");
  assert_eq!(cmap.lookup(b"\x05").as_deref(), Some("\u{1F600}"));
  assert_eq!(cmap.lookup(b"\x06").as_deref(), Some("ffi"));
}

#[test]
fn garbage_and_empty_destinations_do_not_map_or_panic() {
  let empty = ToUnicode::parse(b"beginbfchar <01> <> <02> [ endbfchar <<< % comment\n");
  assert_eq!(
    empty.lookup(b"\x01"),
    None,
    "an empty destination is no mapping"
  );
  assert_eq!(empty.lookup(b"\x02"), None, "a pair cut short is skipped");
  let wild = ToUnicode::parse(b"beginbfrange <00> <FFFFFFFF> <0041> endbfrange");
  assert_eq!(
    wild.lookup(b"\x00").as_deref(),
    Some("A"),
    "a wild range is clamped to the code length"
  );
  assert_eq!(wild.lookup(b"\xff").as_deref(), Some("\u{0140}"));
  assert_eq!(
    wild.lookup(b"\x01\x00"),
    None,
    "and never spills into two-byte codes"
  );
  assert_eq!(ToUnicode::parse(b"").lookup(b"\x41"), None);
}
