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
  let leaf1 = doc.add_object(
    dictionary! { "Names" => vec![Object::string_literal("factur-x.xml"), Object::Reference(xml)] },
  );
  let leaf2 = doc.add_object(
    dictionary! { "Names" => vec![Object::string_literal("readme.txt"), Object::Reference(txt)] },
  );
  let root = doc
    .add_object(dictionary! { "Kids" => vec![Object::Reference(leaf1), Object::Reference(leaf2)] });
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
