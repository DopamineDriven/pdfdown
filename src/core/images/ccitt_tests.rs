use super::*;
use lopdf::dictionary;

/// Pack a string of 0/1 characters (spaces ignored) into bytes, MSB first.
fn bits(s: &str) -> Vec<u8> {
  let mut out = Vec::new();
  let mut acc = 0u8;
  let mut n = 0;
  for c in s.chars().filter(|c| *c == '0' || *c == '1') {
    acc = (acc << 1) | u8::from(c == '1');
    n += 1;
    if n == 8 {
      out.push(acc);
      acc = 0;
      n = 0;
    }
  }
  if n > 0 {
    out.push(acc << (8 - n));
  }
  out
}

fn params(k: i64, columns: u32, rows: u32) -> CcittParams {
  CcittParams {
    k,
    columns,
    rows,
    black_is_1: false,
    encoded_byte_align: false,
    end_of_line: false,
    end_of_block: true,
    inverted: false,
  }
}

const W: u8 = 255;
const B: u8 = 0;

#[test]
fn group4_rows_decode_to_gray_pixels() {
  // Row 1 "WWBBBWWW": horizontal mode (001) white 2 (0111) black 3 (10), then V0 (1).
  // Row 2 all white: pass mode (0001) past the reference changes, then V0 (1).
  // Then EOFB.
  let data = bits("001 0111 10 1  0001 1  000000000001 000000000001");
  let img = decode_ccitt(&data, 8, 2, params(-1, 8, 2)).expect("decodes");
  assert_eq!((img.width(), img.height()), (8, 2));
  let px = img.to_luma8().into_raw();
  assert_eq!(&px[..8], &[W, W, B, B, B, W, W, W]);
  assert_eq!(&px[8..], &[W; 8]);
}

#[test]
fn group3_one_dimensional_rows_decode() {
  // K = 0 (what Ricoh copiers write): modified Huffman runs, no EOLs.
  // Row 1: white 2 (0111) black 3 (10) white 3 (1000). Row 2: white 8 (10011).
  let data = bits("0111 10 1000  10011");
  let img = decode_ccitt(&data, 8, 2, params(0, 8, 2)).expect("decodes");
  let px = img.to_luma8().into_raw();
  assert_eq!(&px[..8], &[W, W, B, B, B, W, W, W]);
  assert_eq!(&px[8..], &[W; 8]);
}

#[test]
fn black_is_1_and_decode_array_flip_polarity_together() {
  let data = bits("0111 10 1000  10011");
  let mut p = params(0, 8, 2);
  p.black_is_1 = true;
  let px = decode_ccitt(&data, 8, 2, p).unwrap().to_luma8().into_raw();
  assert_eq!(
    &px[..8],
    &[B, B, W, W, W, B, B, B],
    "BlackIs1 alone shows inverted"
  );
  p.inverted = true;
  let px = decode_ccitt(&data, 8, 2, p).unwrap().to_luma8().into_raw();
  assert_eq!(
    &px[..8],
    &[W, W, B, B, B, W, W, W],
    "/Decode [1 0] restores it"
  );
}

#[test]
fn truncated_streams_keep_decoded_rows_and_pad_white() {
  let data = bits("0111 10 1000");
  let img = decode_ccitt(&data, 8, 3, params(0, 8, 3)).expect("partial decode");
  assert_eq!(img.height(), 3);
  let px = img.to_luma8().into_raw();
  assert_eq!(&px[..8], &[W, W, B, B, B, W, W, W]);
  assert_eq!(&px[8..], &[W; 16]);
  assert!(decode_ccitt(&[], 8, 2, params(0, 8, 2)).is_none());
}

#[test]
fn params_follow_the_spec_defaults() {
  let doc = Document::with_version("1.7");
  let dict = dictionary! { "Filter" => "CCITTFaxDecode" };
  let p = ccitt_params(&doc, &dict, "CCITTFaxDecode", 2480, 3508).unwrap();
  assert_eq!((p.k, p.columns, p.rows), (0, 2480, 3508));
  assert!(
    !p.black_is_1 && !p.encoded_byte_align && !p.end_of_line && p.end_of_block && !p.inverted
  );
  let dict = dictionary! {
    "Filter" => "CCITTFaxDecode",
    "Decode" => vec![1.into(), 0.into()],
    "DecodeParms" => Object::Dictionary(dictionary! { "K" => -1, "Columns" => 1728, "BlackIs1" => true }),
  };
  let p = ccitt_params(&doc, &dict, "CCITTFaxDecode", 1728, 10).unwrap();
  assert_eq!((p.k, p.columns, p.rows), (-1, 1728, 10));
  assert!(p.black_is_1 && p.inverted);
  assert!(ccitt_params(&doc, &dict, "DCTDecode", 1, 1).is_none());
}
