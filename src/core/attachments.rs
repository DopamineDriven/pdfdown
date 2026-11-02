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
mod tests {
  use super::*;
  use lopdf::{Stream, dictionary};

  fn doc_with_pages(n: usize) -> (Document, Vec<ObjectId>, ObjectId) {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let mut kids = Vec::new();
    let mut page_ids = Vec::new();
    for _ in 0..n {
      let pid = doc.add_object(dictionary! {
        "Type" => "Page", "Parent" => Object::Reference(pages_id),
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
      });
      kids.push(Object::Reference(pid));
      page_ids.push(pid);
    }
    doc.objects.insert(
      pages_id,
      Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => n as i64 }),
    );
    let catalog =
      doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => Object::Reference(pages_id) });
    doc.trailer.set("Root", Object::Reference(catalog));
    (doc, page_ids, catalog)
  }

  fn embedded(
    doc: &mut Document,
    name: &str,
    mime: &str,
    data: &[u8],
    rel: Option<&str>,
  ) -> ObjectId {
    let stream = doc.add_object(Stream::new(
      dictionary! { "Type" => "EmbeddedFile", "Subtype" => Object::Name(mime.as_bytes().to_vec()) },
      data.to_vec(),
    ));
    let mut spec = dictionary! {
      "Type" => "Filespec", "F" => Object::string_literal(name), "UF" => lopdf::text_string(name),
      "EF" => Object::Dictionary(dictionary! { "F" => Object::Reference(stream), "UF" => Object::Reference(stream) }),
    };
    if let Some(r) = rel {
      spec.set("AFRelationship", r);
    }
    doc.add_object(spec)
  }

  fn set_catalog(doc: &mut Document, catalog: ObjectId, key: &str, value: Object) {
    if let Some(Object::Dictionary(d)) = doc.objects.get_mut(&catalog) {
      d.set(key, value);
    }
  }

  #[test]
  fn name_tree_with_kids_and_af_dedupe() {
    let (mut doc, _, catalog) = doc_with_pages(1);
    let xml = embedded(
      &mut doc,
      "factur-x.xml",
      "text/xml",
      b"<x/>",
      Some("Alternative"),
    );
    let txt = embedded(&mut doc, "readme.txt", "text/plain", b"hi", None);
    let leaf1 = doc.add_object(dictionary! { "Names" => vec![Object::string_literal("factur-x.xml"), Object::Reference(xml)] });
    let leaf2 = doc.add_object(
      dictionary! { "Names" => vec![Object::string_literal("readme.txt"), Object::Reference(txt)] },
    );
    let root = doc.add_object(
      dictionary! { "Kids" => vec![Object::Reference(leaf1), Object::Reference(leaf2)] },
    );
    set_catalog(
      &mut doc,
      catalog,
      "Names",
      Object::Dictionary(dictionary! { "EmbeddedFiles" => Object::Reference(root) }),
    );
    set_catalog(
      &mut doc,
      catalog,
      "AF",
      Object::Array(vec![Object::Reference(xml)]),
    );
    let out = extract_attachments(&doc);
    assert_eq!(out.len(), 2, "the /AF entry duplicates a name-tree file");
    assert_eq!(out[0].name, "factur-x.xml");
    assert_eq!(out[0].mime_type.as_deref(), Some("text/xml"));
    assert_eq!(out[0].relationship.as_deref(), Some("Alternative"));
    assert_eq!(out[0].data, b"<x/>");
    assert_eq!(out[1].name, "readme.txt");
    assert!(out[1].page.is_none());
  }

  #[test]
  fn file_attachment_annotations_carry_their_page() {
    let (mut doc, pages, _) = doc_with_pages(2);
    let csv = embedded(&mut doc, "notes.csv", "text/csv", b"a,b", None);
    let annot = doc.add_object(dictionary! {
      "Type" => "Annot", "Subtype" => "FileAttachment", "Rect" => vec![0.into(), 0.into(), 10.into(), 10.into()],
      "FS" => Object::Reference(csv),
    });
    if let Some(Object::Dictionary(p)) = doc.objects.get_mut(&pages[1]) {
      p.set("Annots", vec![Object::Reference(annot)]);
    }
    let out = extract_attachments(&doc);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].page, Some(2));
    assert_eq!(out[0].name, "notes.csv");
  }

  #[test]
  fn compressed_streams_are_decoded_and_mime_escapes_undone() {
    let (mut doc, _, catalog) = doc_with_pages(1);
    let mut stream = Stream::new(
      dictionary! { "Type" => "EmbeddedFile", "Subtype" => Object::Name(b"text#2Fxml".to_vec()) },
      b"<invoice>payload</invoice>".to_vec(),
    );
    stream.compress().unwrap();
    let stream = doc.add_object(stream);
    let spec = doc.add_object(dictionary! {
      "Type" => "Filespec", "F" => Object::string_literal("inv.xml"),
      "EF" => Object::Dictionary(dictionary! { "F" => Object::Reference(stream) }),
    });
    set_catalog(
      &mut doc,
      catalog,
      "AF",
      Object::Array(vec![Object::Reference(spec)]),
    );
    let out = extract_attachments(&doc);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].data, b"<invoice>payload</invoice>");
    assert_eq!(out[0].mime_type.as_deref(), Some("text/xml"));
  }

  #[test]
  fn external_file_specs_and_documents_without_files_yield_nothing() {
    let (mut doc, _, catalog) = doc_with_pages(1);
    let external = doc.add_object(
      dictionary! { "Type" => "Filespec", "F" => Object::string_literal("elsewhere.pdf") },
    );
    set_catalog(
      &mut doc,
      catalog,
      "AF",
      Object::Array(vec![Object::Reference(external)]),
    );
    assert!(extract_attachments(&doc).is_empty());
    let (doc, _, _) = doc_with_pages(1);
    assert!(extract_attachments(&doc).is_empty());
  }

  #[test]
  fn decode_name_handles_escapes_and_latin1() {
    assert_eq!(decode_name(b"text#2Fxml"), "text/xml");
    assert_eq!(decode_name(b"plain"), "plain");
    assert_eq!(decode_name(&[0xE9]), "é");
  }
}
