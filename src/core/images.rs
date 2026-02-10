use crate::types::RawPageImage;
use image::{DynamicImage, ImageBuffer, ImageFormat};
use lopdf::{Document, Object, ObjectId};
use rayon::prelude::*;
use std::collections::HashSet;
use std::io::Cursor;

pub(crate) fn extract_images_raw(doc: &Document) -> Vec<RawPageImage> {
  let pages = doc.get_pages();
  let page_entries: Vec<(u32, ObjectId)> = pages.iter().map(|(&k, &v)| (k, v)).collect();
  let mut results: Vec<RawPageImage> = page_entries
    .par_iter()
    .flat_map(|&(page_num, page_id)| collect_page_images_raw(doc, page_id, page_num))
    .collect();
  results.sort_unstable_by_key(|r| (r.page, r.image_index));
  results
}

/// Decode all image XObjects on a page to DynamicImages (no PNG encoding).
/// Used by OCR to avoid the PNG encode→decode roundtrip.
#[cfg(feature = "ocr")]
pub(crate) fn collect_page_decoded_images(doc: &Document, page_id: ObjectId) -> Vec<DynamicImage> {
  let mut decoded = Vec::new();

  let xobjects = match get_page_xobjects(doc, page_id) {
    Some(x) => x,
    None => return decoded,
  };

  let referenced_names = get_referenced_xobject_names(doc, page_id);

  for (name, obj_ref) in xobjects.iter() {
    if !referenced_names.is_empty() && !referenced_names.contains(name) {
      continue;
    }

    let obj_id = match obj_ref {
      Object::Reference(id) => *id,
      _ => continue,
    };

    let stream = match doc.get_object(obj_id) {
      Ok(Object::Stream(s)) => s,
      _ => continue,
    };

    let subtype = stream.dict.get(b"Subtype").ok().and_then(|v| {
      if let Object::Name(n) = v {
        Some(n.as_slice())
      } else {
        None
      }
    });
    if subtype != Some(b"Image") {
      continue;
    }

    let width = get_dict_int(&stream.dict, b"Width").unwrap_or(0) as u32;
    let height = get_dict_int(&stream.dict, b"Height").unwrap_or(0) as u32;
    let bpc = get_dict_int(&stream.dict, b"BitsPerComponent").unwrap_or(8) as u32;
    if width == 0 || height == 0 {
      continue;
    }

    let color_space = resolve_color_space(&stream.dict, doc);
    let filter = resolve_filter(&stream.dict);

    let content = match filter.as_str() {
      "DCTDecode" | "JPXDecode" => stream.content.clone(),
      _ => {
        let mut s = stream.clone();
        if s.decompress().is_ok() {
          s.content
        } else {
          stream.content.clone()
        }
      }
    };

    // Skip SMask for OCR — to_rgb8() drops alpha anyway
    if let Some(img) =
      decode_xobject_to_dynamic_image(&content, width, height, bpc, &color_space, &filter, None)
    {
      decoded.push(img);
    }
  }

  decoded
}

fn collect_page_images_raw(doc: &Document, page_id: ObjectId, page_num: u32) -> Vec<RawPageImage> {
  let mut images = Vec::new();

  // Get XObjects from page resources (with parent inheritance)
  let xobjects = match get_page_xobjects(doc, page_id) {
    Some(x) => x,
    None => return images,
  };

  // Get the set of XObject names actually referenced by Do operators in the content stream
  let referenced_names = get_referenced_xobject_names(doc, page_id);

  let mut img_index = 0u32;

  for (name, obj_ref) in xobjects.iter() {
    // Only process XObjects actually painted on the page via Do operators
    if !referenced_names.is_empty() && !referenced_names.contains(name) {
      continue;
    }

    let obj_id = match obj_ref {
      Object::Reference(id) => *id,
      _ => continue,
    };

    let stream = match doc.get_object(obj_id) {
      Ok(Object::Stream(s)) => s,
      _ => continue,
    };

    // Only process Image XObjects
    let subtype = stream.dict.get(b"Subtype").ok().and_then(|v| {
      if let Object::Name(n) = v {
        Some(n.as_slice())
      } else {
        None
      }
    });

    if subtype != Some(b"Image") {
      continue;
    }

    let width = get_dict_int(&stream.dict, b"Width").unwrap_or(0) as u32;
    let height = get_dict_int(&stream.dict, b"Height").unwrap_or(0) as u32;
    let bpc = get_dict_int(&stream.dict, b"BitsPerComponent").unwrap_or(8) as u32;

    if width == 0 || height == 0 {
      continue;
    }

    let color_space = resolve_color_space(&stream.dict, doc);
    let filter = resolve_filter(&stream.dict);

    let channels: u32 = match color_space.as_str() {
      "DeviceRGB" | "ICCBased3" | "CalRGB" => 3,
      "DeviceGray" | "ICCBased1" | "CalGray" => 1,
      "DeviceCMYK" | "ICCBased4" => 4,
      _ => 3,
    };

    // Step 4: Skip the full stream clone for DCT/JPX — they're already in their
    // target encoded format and don't need lopdf decompression.
    let content = match filter.as_str() {
      "DCTDecode" | "JPXDecode" => stream.content.clone(),
      _ => decompress_stream_content(doc, stream, width, height, channels, bpc),
    };

    // Check for SMask (alpha channel)
    let smask_data = get_smask_data(doc, &stream.dict);

    let png_data = match encode_to_png(
      &content,
      width,
      height,
      bpc,
      &color_space,
      &filter,
      smask_data.as_deref(),
    ) {
      Some(data) => data,
      None => continue,
    };

    let xobject_name = String::from_utf8_lossy(name).to_string();
    let object_id_str = format!("{} {} obj", obj_id.0, obj_id.1);

    images.push(RawPageImage {
      page: page_num,
      image_index: img_index,
      width,
      height,
      data: png_data,
      color_space,
      bits_per_component: bpc,
      filter,
      xobject_name,
      object_id: object_id_str,
    });

    img_index += 1;
  }

  images
}

/// Walk the page tree to find /Resources (handles inheritance from /Parent)
fn get_page_xobjects(doc: &Document, page_id: ObjectId) -> Option<lopdf::Dictionary> {
  let resources = get_inherited_resources(doc, page_id)?;
  let xobject_obj = resources.get(b"XObject").ok()?;
  resolve_to_dict(doc, xobject_obj)
}

fn get_inherited_resources(doc: &Document, page_id: ObjectId) -> Option<lopdf::Dictionary> {
  let mut current_id = Some(page_id);
  while let Some(id) = current_id {
    let dict = doc.get_dictionary(id).ok()?;
    if let Ok(resources_obj) = dict.get(b"Resources") {
      return resolve_to_dict(doc, resources_obj);
    }
    // Walk up to /Parent
    current_id = dict.get(b"Parent").ok().and_then(|p| match p {
      Object::Reference(ref_id) => Some(*ref_id),
      _ => None,
    });
  }
  None
}

/// Parse the page content stream to find XObject names referenced by `Do` operators.
/// This filters out XObjects that are defined in Resources but never actually painted.
fn get_referenced_xobject_names(doc: &Document, page_id: ObjectId) -> HashSet<Vec<u8>> {
  let mut names = HashSet::new();

  let page_dict = match doc.get_dictionary(page_id) {
    Ok(d) => d,
    Err(_) => return names,
  };

  let contents = match page_dict.get(b"Contents") {
    Ok(c) => c,
    Err(_) => return names,
  };

  let stream_ids: Vec<ObjectId> = match contents {
    Object::Reference(id) => vec![*id],
    Object::Array(arr) => arr
      .iter()
      .filter_map(|o| {
        if let Object::Reference(id) = o {
          Some(*id)
        } else {
          None
        }
      })
      .collect(),
    _ => return names,
  };

  let mut all_bytes = Vec::new();
  for stream_id in stream_ids {
    if let Ok(Object::Stream(s)) = doc.get_object(stream_id) {
      let mut s = s.clone();
      let _ = s.decompress();
      all_bytes.extend_from_slice(&s.content);
    }
  }

  if let Ok(content) = lopdf::content::Content::decode(&all_bytes) {
    for op in &content.operations {
      if op.operator == "Do"
        && let Some(Object::Name(name)) = op.operands.first()
      {
        names.insert(name.clone());
      }
    }
  }

  names
}

/// Resolve /DecodeParms from a stream dictionary, following indirect references.
fn resolve_decode_parms(doc: &Document, dict: &lopdf::Dictionary) -> Option<lopdf::Dictionary> {
  let dp = dict.get(b"DecodeParms").ok()?;
  match dp {
    Object::Dictionary(d) => Some(d.clone()),
    Object::Reference(id) => match doc.get_object(*id) {
      Ok(Object::Dictionary(d)) => Some(d.clone()),
      _ => None,
    },
    Object::Array(arr) => {
      // Filter chain: DecodeParms is an array parallel to Filter array.
      // Use the first dictionary entry found.
      for item in arr {
        match item {
          Object::Dictionary(d) => return Some(d.clone()),
          Object::Reference(id) => {
            if let Ok(Object::Dictionary(d)) = doc.get_object(*id) {
              return Some(d.clone());
            }
          }
          _ => {}
        }
      }
      None
    }
    _ => None,
  }
}

/// Apply PNG predictor unfiltering to raw decompressed data.
/// Each row has a 1-byte filter type prefix followed by `row_bytes` of filtered data.
/// `bytes_per_pixel` is the number of bytes per pixel (channels * ceil(bpc/8)).
fn apply_png_predictor(data: &[u8], bytes_per_pixel: usize, row_bytes: usize) -> Option<Vec<u8>> {
  let src_row_len = row_bytes + 1; // +1 for filter type byte
  if !data.len().is_multiple_of(src_row_len) {
    return None;
  }
  let num_rows = data.len() / src_row_len;
  let mut output = Vec::with_capacity(num_rows * row_bytes);
  let mut prev_row = vec![0u8; row_bytes];

  for row_idx in 0..num_rows {
    let row_start = row_idx * src_row_len;
    let filter_byte = data[row_start];
    let mut current_row = data[row_start + 1..row_start + src_row_len].to_vec();

    match filter_byte {
      0 => { /* None */ }
      1 => {
        // Sub
        for i in bytes_per_pixel..row_bytes {
          current_row[i] = current_row[i].wrapping_add(current_row[i - bytes_per_pixel]);
        }
      }
      2 => {
        // Up
        for i in 0..row_bytes {
          current_row[i] = current_row[i].wrapping_add(prev_row[i]);
        }
      }
      3 => {
        // Average
        for i in 0..bytes_per_pixel {
          current_row[i] = current_row[i].wrapping_add(prev_row[i] / 2);
        }
        for i in bytes_per_pixel..row_bytes {
          current_row[i] = current_row[i].wrapping_add(
            ((current_row[i - bytes_per_pixel] as u16 + prev_row[i] as u16) / 2) as u8,
          );
        }
      }
      4 => {
        // Paeth
        for i in 0..bytes_per_pixel {
          current_row[i] = current_row[i].wrapping_add(paeth_predictor(0, prev_row[i], 0));
        }
        for i in bytes_per_pixel..row_bytes {
          current_row[i] = current_row[i].wrapping_add(paeth_predictor(
            current_row[i - bytes_per_pixel],
            prev_row[i],
            prev_row[i - bytes_per_pixel],
          ));
        }
      }
      _ => return None, // Unknown filter type
    }

    output.extend_from_slice(&current_row);
    prev_row = current_row;
  }

  Some(output)
}

fn paeth_predictor(a: u8, b: u8, c: u8) -> u8 {
  let pa = (b as i16 - c as i16).abs();
  let pb = (a as i16 - c as i16).abs();
  let pc = (a as i16 + b as i16 - 2 * c as i16).abs();
  if pa <= pb && pa <= pc {
    a
  } else if pb <= pc {
    b
  } else {
    c
  }
}

/// Decompress a stream's content with correct predictor handling.
///
/// lopdf's built-in `decompress()` attempts PNG predictor unfiltering internally
/// but produces corrupted output for some streams (e.g. xdvipdfmx/pandoc images).
/// We bypass it entirely: raw zlib inflate via `flate2`, then apply our own
/// predictor reversal.
fn decompress_stream_content(
  doc: &Document,
  stream: &lopdf::Stream,
  width: u32,
  height: u32,
  channels: u32,
  bpc: u32,
) -> Vec<u8> {
  let bytes_per_sample = if bpc > 8 { 2u32 } else { 1u32 };
  let row_bytes = (width * channels * bpc / 8) as usize;
  let expected = (width * height * channels * bytes_per_sample) as usize;
  let predicted_len = height as usize * (row_bytes + 1);

  // Check if the stream uses FlateDecode
  let uses_flate = match stream.dict.get(b"Filter") {
    Ok(Object::Name(n)) => n == b"FlateDecode",
    Ok(Object::Array(arr)) => arr
      .iter()
      .any(|o| matches!(o, Object::Name(n) if n == b"FlateDecode")),
    _ => false,
  };

  // Step 1: Raw inflate — bypass lopdf's decompress to avoid its buggy predictor handling
  let content = if uses_flate {
    raw_inflate(&stream.content).unwrap_or_else(|| {
      // Fallback: let lopdf try (handles edge cases like chained filters)
      let mut s = stream.clone();
      if s.decompress().is_ok() {
        s.content
      } else {
        stream.content.clone()
      }
    })
  } else {
    stream.content.clone()
  };

  // Step 2: Apply predictor reversal if DecodeParms specifies one
  if let Some(dp) = resolve_decode_parms(doc, &stream.dict) {
    let predictor = get_dict_int(&dp, b"Predictor").unwrap_or(1);

    // TIFF Predictor 2: horizontal differencing (same size as raw pixels)
    if predictor == 2 && content.len() == expected {
      let bpp = (channels * bpc / 8).max(1) as usize;
      let mut data = content;
      apply_tiff_predictor2(&mut data, bpp, row_bytes);
      return data;
    }

    // PNG Predictors 10-15: each row has a leading filter type byte
    if (10..=15).contains(&predictor) && content.len() == predicted_len {
      let bpp = (channels * bpc / 8).max(1) as usize;
      if let Some(unfiltered) = apply_png_predictor(&content, bpp, row_bytes) {
        return unfiltered;
      }
    }
  }

  content
}

/// Raw zlib inflate without any predictor handling.
fn raw_inflate(data: &[u8]) -> Option<Vec<u8>> {
  use std::io::Read;
  // Try zlib wrapper first (most common in PDF)
  let mut output = Vec::new();
  if flate2::read::ZlibDecoder::new(data)
    .read_to_end(&mut output)
    .is_ok()
  {
    return Some(output);
  }
  // Fallback to raw deflate (no zlib header)
  output.clear();
  if flate2::read::DeflateDecoder::new(data)
    .read_to_end(&mut output)
    .is_ok()
  {
    return Some(output);
  }
  None
}

/// Reverse TIFF Predictor 2 (horizontal differencing) in-place.
/// Each byte after the first `bpp` bytes in each row is a delta from the previous byte.
fn apply_tiff_predictor2(data: &mut [u8], bpp: usize, row_bytes: usize) {
  if row_bytes == 0 {
    return;
  }
  let num_rows = data.len() / row_bytes;
  for row in 0..num_rows {
    let start = row * row_bytes;
    for i in (start + bpp)..(start + row_bytes) {
      data[i] = data[i].wrapping_add(data[i - bpp]);
    }
  }
}

/// Retrieve and decompress the SMask (soft mask / alpha channel) image data if present
fn get_smask_data(doc: &Document, dict: &lopdf::Dictionary) -> Option<Vec<u8>> {
  let smask_ref = dict.get(b"SMask").ok()?;
  let smask_id = match smask_ref {
    Object::Reference(id) => *id,
    _ => return None,
  };

  let smask_stream = match doc.get_object(smask_id) {
    Ok(Object::Stream(s)) => s,
    _ => return None,
  };

  // Verify it's an Image subtype
  let subtype = smask_stream.dict.get(b"Subtype").ok().and_then(|v| {
    if let Object::Name(n) = v {
      Some(n.as_slice())
    } else {
      None
    }
  });
  if subtype != Some(b"Image") {
    return None;
  }

  // SMask is always DeviceGray with 1 channel
  let smask_width = get_dict_int(&smask_stream.dict, b"Width").unwrap_or(0) as u32;
  let smask_height = get_dict_int(&smask_stream.dict, b"Height").unwrap_or(0) as u32;
  let smask_bpc = get_dict_int(&smask_stream.dict, b"BitsPerComponent").unwrap_or(8) as u32;
  Some(decompress_stream_content(
    doc,
    smask_stream,
    smask_width,
    smask_height,
    1,
    smask_bpc,
  ))
}

fn resolve_to_dict(doc: &Document, obj: &Object) -> Option<lopdf::Dictionary> {
  match obj {
    Object::Dictionary(d) => Some(d.clone()),
    Object::Reference(id) => match doc.get_object(*id).ok()? {
      Object::Dictionary(d) => Some(d.clone()),
      _ => None,
    },
    _ => None,
  }
}

fn get_dict_int(dict: &lopdf::Dictionary, key: &[u8]) -> Option<i64> {
  match dict.get(key).ok()? {
    Object::Integer(i) => Some(*i),
    _ => None,
  }
}

fn resolve_color_space(dict: &lopdf::Dictionary, doc: &Document) -> String {
  let cs = match dict.get(b"ColorSpace") {
    Ok(obj) => obj,
    Err(_) => return "DeviceRGB".to_string(),
  };

  match cs {
    Object::Name(name) => String::from_utf8_lossy(name).to_string(),
    Object::Reference(id) => match doc.get_object(*id) {
      Ok(Object::Name(name)) => String::from_utf8_lossy(name).to_string(),
      // ICCBased is typically [/ICCBased <stream ref>]
      Ok(Object::Array(arr)) => parse_color_space_array(arr, doc),
      _ => "DeviceRGB".to_string(),
    },
    Object::Array(arr) => parse_color_space_array(arr, doc),
    _ => "DeviceRGB".to_string(),
  }
}

fn parse_color_space_array(arr: &[Object], doc: &Document) -> String {
  if arr.is_empty() {
    return "DeviceRGB".to_string();
  }

  let cs_name = match &arr[0] {
    Object::Name(n) => String::from_utf8_lossy(n).to_string(),
    _ => return "DeviceRGB".to_string(),
  };

  if cs_name == "ICCBased" && arr.len() > 1 {
    // Get /N from the ICCBased stream to determine channel count
    let stream_id = match &arr[1] {
      Object::Reference(id) => *id,
      _ => return "ICCBased".to_string(),
    };

    if let Ok(Object::Stream(s)) = doc.get_object(stream_id) {
      let n = get_dict_int(&s.dict, b"N").unwrap_or(3);
      return format!("ICCBased{n}");
    }
  }

  cs_name
}

fn resolve_filter(dict: &lopdf::Dictionary) -> String {
  match dict.get(b"Filter") {
    Ok(Object::Name(name)) => String::from_utf8_lossy(name).to_string(),
    Ok(Object::Array(arr)) => {
      // Filter chain — return the last (innermost) filter for image type detection
      if let Some(Object::Name(name)) = arr.last() {
        String::from_utf8_lossy(name).to_string()
      } else {
        "None".to_string()
      }
    }
    _ => "None".to_string(),
  }
}

/// Decode an XObject stream into a DynamicImage (shared by PNG export and OCR).
fn decode_xobject_to_dynamic_image(
  content: &[u8],
  width: u32,
  height: u32,
  bpc: u32,
  color_space: &str,
  filter: &str,
  smask: Option<&[u8]>,
) -> Option<DynamicImage> {
  let dynamic_img = if filter == "DCTDecode" {
    image::load_from_memory_with_format(content, ImageFormat::Jpeg).ok()?
  } else if filter == "JPXDecode" {
    decode_jpx(content)?
  } else {
    decode_raw_pixels(content, width, height, bpc, color_space)?
  };

  Some(if let Some(mask_data) = smask {
    apply_smask(dynamic_img, mask_data, width, height)
  } else {
    dynamic_img
  })
}

fn encode_to_png(
  content: &[u8],
  width: u32,
  height: u32,
  bpc: u32,
  color_space: &str,
  filter: &str,
  smask: Option<&[u8]>,
) -> Option<Vec<u8>> {
  let final_img =
    decode_xobject_to_dynamic_image(content, width, height, bpc, color_space, filter, smask)?;
  let mut png_buf = Cursor::new(Vec::new());
  final_img.write_to(&mut png_buf, ImageFormat::Png).ok()?;
  Some(png_buf.into_inner())
}

/// Decode a JPEG 2000 (JPXDecode) stream using hayro-jpeg2000 (pure Rust)
fn decode_jpx(content: &[u8]) -> Option<DynamicImage> {
  let jp2_img =
    hayro_jpeg2000::Image::new(content, &hayro_jpeg2000::DecodeSettings::default()).ok()?;
  DynamicImage::from_decoder(jp2_img).ok()
}

/// Decode raw pixel data (FlateDecode / uncompressed) into a DynamicImage
fn decode_raw_pixels(
  content: &[u8],
  width: u32,
  height: u32,
  bpc: u32,
  color_space: &str,
) -> Option<DynamicImage> {
  let channels: u32 = match color_space {
    "DeviceRGB" | "ICCBased3" | "CalRGB" => 3,
    "DeviceGray" | "ICCBased1" | "CalGray" => 1,
    "DeviceCMYK" | "ICCBased4" => 4,
    _ => 3,
  };
  let bytes_per_sample = if bpc > 8 { 2u32 } else { 1u32 };
  let expected = (width * height * channels * bytes_per_sample) as usize;

  // Validate buffer size before constructing image
  if content.len() < expected {
    return None;
  }
  // Use exactly the expected number of bytes
  let pixel_data = &content[..expected];

  // Downscale 16-bit to 8-bit if needed
  let pixel_data_8bit: Vec<u8> = if bytes_per_sample == 2 {
    pixel_data
      .chunks_exact(2)
      .map(|pair| (u16::from_be_bytes([pair[0], pair[1]]) >> 8) as u8)
      .collect()
  } else {
    pixel_data.to_vec()
  };

  match color_space {
    "DeviceRGB" | "ICCBased3" | "CalRGB" => {
      let img: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, pixel_data_8bit)?;
      Some(DynamicImage::ImageRgb8(img))
    }
    "DeviceGray" | "ICCBased1" | "CalGray" => {
      let img: ImageBuffer<image::Luma<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, pixel_data_8bit)?;
      Some(DynamicImage::ImageLuma8(img))
    }
    "DeviceCMYK" | "ICCBased4" => {
      let rgb_bytes = cmyk_to_rgb(&pixel_data_8bit);
      let img: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, rgb_bytes)?;
      Some(DynamicImage::ImageRgb8(img))
    }
    _ => {
      let expected_rgb = (width * height * 3) as usize;
      if pixel_data_8bit.len() == expected_rgb {
        let img: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
          ImageBuffer::from_raw(width, height, pixel_data_8bit)?;
        Some(DynamicImage::ImageRgb8(img))
      } else {
        None
      }
    }
  }
}

/// Combine a base RGB image with a grayscale SMask to produce an RGBA PNG
fn apply_smask(base: DynamicImage, mask_data: &[u8], width: u32, height: u32) -> DynamicImage {
  let rgb = base.to_rgb8();
  let expected_mask_len = (width * height) as usize;

  if mask_data.len() < expected_mask_len {
    return DynamicImage::ImageRgb8(rgb);
  }

  let mut rgba_pixels = Vec::with_capacity((width * height * 4) as usize);
  for (rgb_pixel, &alpha) in rgb.pixels().zip(mask_data.iter()) {
    rgba_pixels.push(rgb_pixel[0]);
    rgba_pixels.push(rgb_pixel[1]);
    rgba_pixels.push(rgb_pixel[2]);
    rgba_pixels.push(alpha);
  }

  match ImageBuffer::from_raw(width, height, rgba_pixels) {
    Some(img) => DynamicImage::ImageRgba8(img),
    None => DynamicImage::ImageRgb8(rgb),
  }
}

fn cmyk_to_rgb(cmyk: &[u8]) -> Vec<u8> {
  let pixel_count = cmyk.len() / 4;
  let mut rgb = Vec::with_capacity(pixel_count * 3);

  for i in 0..pixel_count {
    let c = cmyk[i * 4] as f32 / 255.0;
    let m = cmyk[i * 4 + 1] as f32 / 255.0;
    let y = cmyk[i * 4 + 2] as f32 / 255.0;
    let k = cmyk[i * 4 + 3] as f32 / 255.0;

    let r = 255.0 * (1.0 - c) * (1.0 - k);
    let g = 255.0 * (1.0 - m) * (1.0 - k);
    let b = 255.0 * (1.0 - y) * (1.0 - k);

    rgb.push(r as u8);
    rgb.push(g as u8);
    rgb.push(b as u8);
  }

  rgb
}
