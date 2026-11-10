//! Embedded files: the `/Names /EmbeddedFiles` name tree, the catalog `/AF`
//! associated-files array, and page-level `/FileAttachment` annotations
//! (ISO 32000-1 §7.11, §12.5.6.15; PDF/A-3 associated files).

use crate::types::RawAttachment;
use lopdf::{Dictionary, Document, Object, ObjectId};
use std::collections::HashSet;

/// Per-attachment decode bound. Embedded invoices are kilobytes; this stops a
/// decompression bomb from taking the process with it.
const MAX_ATTACHMENT_BYTES: usize = 64 * 1024 * 1024;
const MAX_TREE_DEPTH: usize = 64;

fn deref<'a>(doc: &'a Document, obj: &'a Object) -> Option<&'a Object> {
  match obj {
    Object::Reference(id) => doc.get_object(*id).ok(),
    other => Some(other),
  }
}

fn as_dict<'a>(doc: &'a Document, obj: &'a Object) -> Option<&'a Dictionary> {
  match deref(doc, obj)? {
    Object::Dictionary(d) => Some(d),
    Object::Stream(s) => Some(&s.dict),
    _ => None,
  }
}

fn text_of(doc: &Document, dict: &Dictionary, key: &[u8]) -> Option<String> {
  let obj = deref(doc, dict.get(key).ok()?)?;
  match obj {
    Object::String(bytes, _) => Some(
      lopdf::decode_text_string(obj).unwrap_or_else(|_| String::from_utf8_lossy(bytes).to_string()),
    ),
    Object::Name(n) => Some(decode_name(n)),
    _ => None,
  }
}

/// PDF name bytes to text; `#XX` escapes are undone when a producer left them in.
fn decode_name(bytes: &[u8]) -> String {
  let mut out = Vec::with_capacity(bytes.len());
  let mut i = 0;
  while i < bytes.len() {
    if bytes[i] == b'#'
      && i + 2 < bytes.len()
      && let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2]))
    {
      out.push(h << 4 | l);
      i += 3;
    } else {
      out.push(bytes[i]);
      i += 1;
    }
  }
  String::from_utf8(out.clone()).unwrap_or_else(|_| out.iter().map(|&b| b as char).collect())
}

fn hex_val(b: u8) -> Option<u8> {
  match b {
    b'0'..=b'9' => Some(b - b'0'),
    b'a'..=b'f' => Some(b - b'a' + 10),
    b'A'..=b'F' => Some(b - b'A' + 10),
    _ => None,
  }
}

/// Identity used to avoid reporting the same file twice when it is reachable
/// from both the name tree and `/AF`.
#[derive(Hash, PartialEq, Eq)]
enum FileKey {
  Object(ObjectId),
  Inline(String, usize),
}

struct Collector<'a> {
  doc: &'a Document,
  seen: HashSet<FileKey>,
  out: Vec<RawAttachment>,
}

impl Collector<'_> {
  /// Read one file specification dictionary. `fallback_name` is the name-tree
  /// key when the dictionary has neither `/UF` nor `/F`.
  fn filespec(&mut self, spec: &Object, fallback_name: Option<&str>, page: Option<u32>) {
    let Some(dict) = as_dict(self.doc, spec) else {
      return;
    };
    let ef = dict.get(b"EF").ok().and_then(|o| as_dict(self.doc, o));
    let Some(ef) = ef else {
      return; // a file specification without embedded data is an external reference
    };
    let stream_obj = ef.get(b"UF").or_else(|_| ef.get(b"F")).ok();
    let Some(stream_obj) = stream_obj else {
      return;
    };
    let stream = match deref(self.doc, stream_obj) {
      Some(Object::Stream(s)) => s,
      _ => return,
    };

    let name = text_of(self.doc, dict, b"UF")
      .or_else(|| text_of(self.doc, dict, b"F"))
      .or_else(|| fallback_name.map(str::to_string))
      .unwrap_or_default();

    let data = stream
      .decompressed_content_with_limit(MAX_ATTACHMENT_BYTES)
      .unwrap_or_default();

    let key = match spec {
      Object::Reference(id) => FileKey::Object(*id),
      _ => FileKey::Inline(name.clone(), data.len()),
    };
    if !self.seen.insert(key) {
      return;
    }

    let mime_type = stream
      .dict
      .get(b"Subtype")
      .ok()
      .and_then(|o| deref(self.doc, o))
      .and_then(|o| o.as_name().ok())
      .map(decode_name);

    self.out.push(RawAttachment {
      name,
      mime_type,
      relationship: text_of(self.doc, dict, b"AFRelationship"),
      description: text_of(self.doc, dict, b"Desc"),
      page,
      data,
    });
  }

  /// Walk a name tree node: `/Names [k1 v1 k2 v2 ...]` leaves and `/Kids` intermediates.
  fn name_tree(&mut self, node: &Object, depth: usize, visited: &mut HashSet<ObjectId>) {
    if depth > MAX_TREE_DEPTH {
      return;
    }
    if let Object::Reference(id) = node
      && !visited.insert(*id)
    {
      return;
    }
    let Some(dict) = as_dict(self.doc, node) else {
      return;
    };
    if let Some(Object::Array(pairs)) = dict.get(b"Names").ok().and_then(|o| deref(self.doc, o)) {
      for pair in pairs.chunks(2) {
        let key = pair.first().and_then(|k| match deref(self.doc, k)? {
          Object::String(bytes, _) => Some(
            lopdf::decode_text_string(deref(self.doc, k)?)
              .unwrap_or_else(|_| String::from_utf8_lossy(bytes).to_string()),
          ),
          _ => None,
        });
        if let Some(value) = pair.get(1) {
          self.filespec(value, key.as_deref(), None);
        }
      }
    }
    if let Some(Object::Array(kids)) = dict.get(b"Kids").ok().and_then(|o| deref(self.doc, o)) {
      for kid in kids {
        self.name_tree(kid, depth + 1, visited);
      }
    }
  }
}

/// Every embedded file in the document, name-tree files first, then `/AF`
/// entries not already seen, then page attachments in page order.
pub(crate) fn extract_attachments(doc: &Document) -> Vec<RawAttachment> {
  let mut c = Collector {
    doc,
    seen: HashSet::new(),
    out: Vec::new(),
  };

  if let Ok(catalog) = doc.catalog() {
    if let Some(names) = catalog.get(b"Names").ok().and_then(|o| as_dict(doc, o))
      && let Ok(tree) = names.get(b"EmbeddedFiles")
    {
      let mut visited = HashSet::new();
      c.name_tree(tree, 0, &mut visited);
    }
    if let Some(Object::Array(af)) = catalog.get(b"AF").ok().and_then(|o| deref(doc, o)) {
      for spec in af {
        c.filespec(spec, None, None);
      }
    }
  }

  for (page_num, page_id) in doc.get_pages() {
    let Ok(page) = doc.get_dictionary(page_id) else {
      continue;
    };
    let Some(Object::Array(annots)) = page.get(b"Annots").ok().and_then(|o| deref(doc, o)) else {
      continue;
    };
    for annot in annots {
      let Some(dict) = as_dict(doc, annot) else {
        continue;
      };
      let is_file = dict
        .get(b"Subtype")
        .ok()
        .and_then(|o| o.as_name().ok())
        .is_some_and(|n| n == b"FileAttachment");
      if !is_file {
        continue;
      }
      if let Ok(fs) = dict.get(b"FS") {
        c.filespec(fs, None, Some(page_num));
      }
    }
  }

  c.out
}

#[cfg(test)]
mod tests;
