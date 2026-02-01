#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;

use image::{DynamicImage, ImageBuffer, ImageFormat};
use lopdf::{Document, Object, ObjectId};
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
    results.push(PageText { page: page_num, text });
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

  let xobjects = match get_page_xobjects(doc, page_id) {
    Some(x) => x,
    None => return images,
  };

  let mut img_index = 0u32;

  for (_name, obj_ref) in xobjects.iter() {
    let obj_id = match obj_ref {
      Object::Reference(id) => *id,
      _ => continue,
    };

    let stream = match doc.get_object(obj_id) {
      Ok(Object::Stream(s)) => s,
      _ => continue,
    };

    // Only process Image XObjects
    let subtype = stream
      .dict
      .get(b"Subtype")
      .ok()
      .and_then(|v| {
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

    // Get decompressed content
    let content = {
      let mut s = stream.clone();
      if s.decompress().is_ok() {
        s.content
      } else {
        stream.content.clone()
      }
    };

    let png_data = match encode_to_png(&content, width, height, bpc, &color_space, &filter) {
      Some(data) => data,
      None => continue,
    };

    images.push(PageImage {
      page: page_num,
      image_index: img_index,
      width,
      height,
      data: png_data.into(),
      color_space,
      bits_per_component: bpc,
      filter,
    });

    img_index += 1;
  }

  images
}

fn get_page_xobjects(
  doc: &Document,
  page_id: ObjectId,
) -> Option<lopdf::Dictionary> {
  let page_dict = doc.get_dictionary(page_id).ok()?;

  // Resources may be a direct dict or a reference
  let resources_obj = page_dict.get(b"Resources").ok()?;
  let resources = resolve_to_dict(doc, resources_obj)?;

  let xobject_obj = resources.get(b"XObject").ok()?;
  resolve_to_dict(doc, xobject_obj)
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
    Object::Reference(id) => {
      match doc.get_object(*id) {
        Ok(Object::Name(name)) => String::from_utf8_lossy(name).to_string(),
        // ICCBased is typically [/ICCBased <stream ref>]
        Ok(Object::Array(arr)) => parse_color_space_array(arr, doc),
        _ => "DeviceRGB".to_string(),
      }
    }
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
      // Filter chain — return the first (outermost) filter
      if let Some(Object::Name(name)) = arr.first() {
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
) -> Option<Vec<u8>> {
  let dynamic_img = if filter == "DCTDecode" {
    // Content is raw JPEG bytes — decode directly
    image::load_from_memory_with_format(content, ImageFormat::Jpeg).ok()?
  } else {
    // Content is raw pixel data (already decompressed by lopdf)
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
        DynamicImage::ImageRgb8(img)
      }
      "DeviceGray" | "ICCBased1" => {
        let img: ImageBuffer<image::Luma<u8>, Vec<u8>> =
          ImageBuffer::from_raw(width, height, pixel_data_8bit)?;
        DynamicImage::ImageLuma8(img)
      }
      "DeviceCMYK" | "ICCBased4" => {
        let rgb_bytes = cmyk_to_rgb(&pixel_data_8bit);
        let img: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
          ImageBuffer::from_raw(width, height, rgb_bytes)?;
        DynamicImage::ImageRgb8(img)
      }
      _ => {
        let expected_rgb = (width * height * 3) as usize;
        if pixel_data_8bit.len() == expected_rgb {
          let img: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_raw(width, height, pixel_data_8bit)?;
          DynamicImage::ImageRgb8(img)
        } else {
          return None;
        }
      }
    }
  };

  let mut png_buf = Cursor::new(Vec::new());
  dynamic_img.write_to(&mut png_buf, ImageFormat::Png).ok()?;
  Some(png_buf.into_inner())
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
