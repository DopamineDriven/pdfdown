#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;

use image::{DynamicImage, ImageBuffer, ImageFormat};
use lopdf::{Document, Object, ObjectId};
use std::collections::HashSet;
use std::io::Cursor;

// ── Text extraction ──────────────────────────────────────────────

#[napi(object)]
pub struct PageText {
  pub page: u32,
  pub text: String,
}

#[napi]
pub fn extract_text_per_page(buffer: Buffer) -> Result<Vec<PageText>> {
  let bytes: &[u8] = buffer.as_ref();
  let doc = Document::load_mem(bytes)
    .map_err(|e| Error::from_reason(format!("Failed to load PDF: {e}")))?;

  let pages = doc.get_pages();
  let mut results = Vec::with_capacity(pages.len());

  for &page_num in pages.keys() {
    let text = doc.extract_text(&[page_num]).unwrap_or_default();
    results.push(PageText {
      page: page_num,
      text,
    });
  }

  Ok(results)
}

// ── Metadata ─────────────────────────────────────────────────────

#[napi(object)]
pub struct PdfMeta {
  pub page_count: u32,
  pub version: String,
  pub is_linearized: bool,
}

#[napi]
pub fn pdf_metadata(buffer: Buffer) -> Result<PdfMeta> {
  let bytes: &[u8] = buffer.as_ref();
  let doc = Document::load_mem(bytes)
    .map_err(|e| Error::from_reason(format!("Failed to load PDF: {e}")))?;

  let page_count = doc.get_pages().len() as u32;
  let version = doc.version.clone();
  let is_linearized = doc.trailer.get(b"Linearized").is_ok();

  Ok(PdfMeta {
    page_count,
    version,
    is_linearized,
  })
}

// ── Image extraction ─────────────────────────────────────────────

#[napi(object)]
pub struct PageImage {
  pub page: u32,
  pub image_index: u32,
  pub width: u32,
  pub height: u32,
  pub data: Buffer,
  pub color_space: String,
  pub bits_per_component: u32,
  pub filter: String,
  pub xobject_name: String,
  pub object_id: String,
}

#[napi]
pub fn extract_images_per_page(buffer: Buffer) -> Result<Vec<PageImage>> {
  let bytes: &[u8] = buffer.as_ref();
  let doc = Document::load_mem(bytes)
    .map_err(|e| Error::from_reason(format!("Failed to load PDF: {e}")))?;

  let pages = doc.get_pages();
  let mut results = Vec::new();

  for (&page_num, &page_id) in &pages {
    let page_images = collect_page_images(&doc, page_id, page_num);
    results.extend(page_images);
  }

  Ok(results)
}

fn collect_page_images(doc: &Document, page_id: ObjectId, page_num: u32) -> Vec<PageImage> {
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

    // Get stream content — for DCT/JPX, preserve raw encoded bytes;
    // for everything else, let lopdf decompress (e.g. FlateDecode → raw pixels)
    let content = {
      let mut s = stream.clone();
      if s.decompress().is_ok() {
        s.content
      } else {
        stream.content.clone()
      }
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

    images.push(PageImage {
      page: page_num,
      image_index: img_index,
      width,
      height,
      data: png_data.into(),
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
      if s.decompress().is_ok() {
        all_bytes.extend_from_slice(&s.content);
      } else {
        all_bytes.extend_from_slice(&s.content);
      }
    }
  }

  if let Ok(content) = lopdf::content::Content::decode(&all_bytes) {
    for op in &content.operations {
      if op.operator == "Do" {
        if let Some(Object::Name(name)) = op.operands.first() {
          names.insert(name.clone());
        }
      }
    }
  }

  names
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

  let mut s = smask_stream.clone();
  if s.decompress().is_ok() {
    Some(s.content)
  } else {
    Some(smask_stream.content.clone())
  }
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

fn encode_to_png(
  content: &[u8],
  width: u32,
  height: u32,
  bpc: u32,
  color_space: &str,
  filter: &str,
  smask: Option<&[u8]>,
) -> Option<Vec<u8>> {
  let dynamic_img = if filter == "DCTDecode" {
    // Content is raw JPEG bytes — decode directly
    image::load_from_memory_with_format(content, ImageFormat::Jpeg).ok()?
  } else if filter == "JPXDecode" {
    // Content is JPEG 2000 codestream — decode with hayro
    decode_jpx(content)?
  } else {
    // Content is raw pixel data (already decompressed by lopdf)
    decode_raw_pixels(content, width, height, bpc, color_space)?
  };

  // Apply SMask alpha channel if present
  let final_img = if let Some(mask_data) = smask {
    apply_smask(dynamic_img, mask_data, width, height)
  } else {
    dynamic_img
  };

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
    "DeviceRGB" | "ICCBased3" => 3,
    "DeviceGray" | "ICCBased1" => 1,
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
    "DeviceRGB" | "ICCBased3" => {
      let img: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, pixel_data_8bit)?;
      Some(DynamicImage::ImageRgb8(img))
    }
    "DeviceGray" | "ICCBased1" => {
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

// ── Legacy ───────────────────────────────────────────────────────

#[napi]
pub fn plus_100(input: u32) -> u32 {
  input + 100
}
