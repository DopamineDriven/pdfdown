//! Generates the AcroForm test fixtures under `__test__/fixtures/forms/` (plan: notes/phase-1.md,
//! Task 5) and the e-invoice attachment fixtures under `__test__/fixtures/einvoice/`. Deterministic, pure lopdf, our own code. Re-run with `cargo run --example gen_fixtures`
//! and commit the output together with any test that depends on it.

use image::{GrayImage, imageops};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, StringFormat, dictionary};
use std::path::{Path, PathBuf};

fn n(name: &str) -> Object {
  Object::Name(name.as_bytes().to_vec())
}

fn s(text: &str) -> Object {
  Object::string_literal(text)
}

fn int(v: i64) -> Object {
  Object::Integer(v)
}

fn rect(a: i64, b: i64, c: i64, d: i64) -> Object {
  Object::Array(vec![int(a), int(b), int(c), int(d)])
}

fn refs(ids: &[ObjectId]) -> Object {
  Object::Array(ids.iter().map(|&id| Object::Reference(id)).collect())
}

/// One PDF under construction: reserved page ids, per-page annotation lists, then `finish`.
struct Pdf {
  doc: Document,
  pages_id: ObjectId,
  page_ids: Vec<ObjectId>,
  page_annots: Vec<Vec<Object>>,
  indirect_annots: Vec<bool>,
  /// Extra catalog entries (`/Names`, `/AF`, …) merged in by `finish`.
  catalog_extra: Dictionary,
  /// Extra entries per page (`/Contents`, `/Resources`, `/Rotate`, …) merged in by `finish`.
  page_extra: Vec<Dictionary>,
}

impl Pdf {
  fn new(page_count: usize) -> Self {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let page_ids = (0..page_count).map(|_| doc.new_object_id()).collect();
    Pdf {
      doc,
      pages_id,
      page_ids,
      page_annots: vec![Vec::new(); page_count],
      indirect_annots: vec![false; page_count],
      catalog_extra: Dictionary::new(),
      page_extra: vec![Dictionary::new(); page_count],
    }
  }

  /// Set one entry on page `page` (0-based); written out by `finish`.
  fn set_page(&mut self, page: usize, key: &str, value: Object) {
    self.page_extra[page].set(key, value);
  }

  fn reserve(&mut self) -> ObjectId {
    self.doc.new_object_id()
  }

  fn put(&mut self, id: ObjectId, obj: impl Into<Object>) {
    self.doc.objects.insert(id, obj.into());
  }

  fn add(&mut self, obj: impl Into<Object>) -> ObjectId {
    self.doc.add_object(obj)
  }

  /// Empty appearance stream: enough for a viewer to find the state name, nothing to draw.
  fn appearance(&mut self) -> ObjectId {
    self.add(Stream::new(
      dictionary! { "Type" => n("XObject"), "Subtype" => n("Form"), "BBox" => rect(0, 0, 20, 20) },
      Vec::new(),
    ))
  }

  /// `/AP << /N << /<state> stream /Off stream >> >>` for each given on-state.
  fn ap(&mut self, states: &[&str]) -> Object {
    let mut normal = Dictionary::new();
    for state in states {
      let stream = self.appearance();
      normal.set(state.as_bytes().to_vec(), Object::Reference(stream));
    }
    let off = self.appearance();
    normal.set("Off", Object::Reference(off));
    dictionary! { "N" => Object::Dictionary(normal) }.into()
  }

  fn widget(dict: &mut Dictionary, r: Object) {
    dict.set("Type", n("Annot"));
    dict.set("Subtype", n("Widget"));
    dict.set("F", int(4));
    dict.set("Rect", r);
  }

  /// A merged field+widget on `page` (0-based). Returns its id.
  fn merged(&mut self, page: usize, mut dict: Dictionary, r: Object) -> ObjectId {
    Self::widget(&mut dict, r);
    let id = self.add(dict);
    self.page_annots[page].push(Object::Reference(id));
    id
  }

  /// A plain Tx field+widget.
  fn text(&mut self, page: usize, name: &str, value: Object, r: Object) -> ObjectId {
    self.merged(
      page,
      dictionary! { "T" => s(name), "FT" => n("Tx"), "V" => value },
      r,
    )
  }

  fn finish(mut self, acroform: Option<Object>, path: &Path) {
    let page_ids = self.page_ids.clone();
    for (k, &pid) in page_ids.iter().enumerate() {
      let annots = Object::Array(std::mem::take(&mut self.page_annots[k]));
      let annots = if self.indirect_annots[k] {
        Object::Reference(self.add(annots))
      } else {
        annots
      };
      let pages_id = self.pages_id;
      let mut page = dictionary! {
        "Type" => n("Page"),
        "Parent" => Object::Reference(pages_id),
        "MediaBox" => rect(0, 0, 612, 792),
        "Annots" => annots
      };
      // `set` per key: lopdf's `Dictionary::extend` merges array values instead of replacing them.
      let extra = std::mem::take(&mut self.page_extra[k]);
      for (key, value) in extra.iter() {
        page.set(key.clone(), value.clone());
      }
      self.put(pid, page);
    }
    let count = page_ids.len() as i64;
    let pages_id = self.pages_id;
    self.put(
      pages_id,
      dictionary! { "Type" => n("Pages"), "Kids" => refs(&page_ids), "Count" => int(count) },
    );
    let mut catalog =
      dictionary! { "Type" => n("Catalog"), "Pages" => Object::Reference(pages_id) };
    if let Some(af) = acroform {
      catalog.set("AcroForm", af);
    }
    let extra = std::mem::take(&mut self.catalog_extra);
    for (key, value) in extra.iter() {
      catalog.set(key.clone(), value.clone());
    }
    let catalog_id = self.add(catalog);
    self.doc.trailer.set("Root", Object::Reference(catalog_id));
    self.doc.save(path).expect("write fixture");
    println!("wrote {}", path.display());
  }
}

fn acroform(fields: &[ObjectId], need_appearances: bool) -> Object {
  dictionary! {
    "Fields" => refs(fields),
    "DA" => s("/Helv 0 Tf 0 g"),
    "NeedAppearances" => Object::Boolean(need_appearances)
  }
  .into()
}

fn basic(out: &Path) {
  let mut p = Pdf::new(1);
  let mut fields = Vec::new();

  fields.push(p.merged(
    0,
    dictionary! { "T" => s("name"), "FT" => n("Tx"), "V" => s("Jane Doe"), "TU" => s("Full name"), "TM" => s("fullName") },
    rect(50, 700, 300, 720),
  ));
  fields.push(p.merged(
    0,
    dictionary! { "T" => s("notes"), "FT" => n("Tx"), "Ff" => int(1 << 12), "MaxLen" => int(200), "V" => lopdf::text_string("Line one\nLine two") },
    rect(50, 600, 500, 680),
  ));
  fields.push(p.merged(
    0,
    dictionary! { "T" => s("zip"), "FT" => n("Tx"), "Ff" => int(1 << 24), "MaxLen" => int(5), "V" => s("60601") },
    rect(50, 560, 150, 580),
  ));
  let ap = p.ap(&["Ja"]);
  fields.push(p.merged(
    0,
    dictionary! { "T" => s("agree"), "FT" => n("Btn"), "Ff" => int(1 << 1), "V" => n("Ja"), "AS" => n("Ja"), "AP" => ap },
    rect(50, 520, 70, 540),
  ));

  // Radio group: one logical field, three unnamed widget kids with /Opt export values.
  let group = p.reserve();
  let mut kids = Vec::new();
  let rects = [
    rect(50, 480, 70, 500),
    rect(100, 480, 120, 500),
    rect(150, 480, 170, 500),
  ];
  for (i, r) in rects.into_iter().enumerate() {
    let state = i.to_string();
    let ap = p.ap(&[&state]);
    let as_state = if i == 1 { "1" } else { "Off" };
    let mut kid =
      dictionary! { "Parent" => Object::Reference(group), "AS" => n(as_state), "AP" => ap };
    Pdf::widget(&mut kid, r);
    let id = p.add(kid);
    p.page_annots[0].push(Object::Reference(id));
    kids.push(id);
  }
  p.put(
    group,
    dictionary! {
      "T" => s("color"),
      "FT" => n("Btn"),
      "Ff" => int((1 << 15) | (1 << 14)),
      "Opt" => Object::Array(vec![s("Red"), s("Green"), s("Blue")]),
      "V" => n("1"),
      "Kids" => refs(&kids)
    },
  );
  fields.push(group);

  fields.push(p.merged(
    0,
    dictionary! {
      "T" => s("country"), "FT" => n("Ch"), "Ff" => int((1 << 17) | (1 << 18)),
      "Opt" => Object::Array(vec![
        Object::Array(vec![s("US"), s("United States")]),
        Object::Array(vec![s("DE"), s("Germany")])
      ]),
      "V" => s("US")
    },
    rect(50, 440, 250, 460),
  ));
  fields.push(p.merged(
    0,
    dictionary! {
      "T" => s("tags"), "FT" => n("Ch"), "Ff" => int(1 << 21),
      "Opt" => Object::Array(vec![s("a"), s("b"), s("c")]),
      "V" => Object::Array(vec![s("a"), s("c")]),
      "I" => Object::Array(vec![int(0), int(2)])
    },
    rect(50, 340, 250, 420),
  ));
  fields.push(p.merged(
    0,
    dictionary! { "T" => s("submit"), "FT" => n("Btn"), "Ff" => int(1 << 16), "A" => Object::Dictionary(dictionary! { "S" => n("ResetForm") }) },
    rect(50, 300, 150, 320),
  ));
  fields.push(p.merged(
    0,
    dictionary! { "T" => s("sig"), "FT" => n("Sig") },
    rect(300, 300, 500, 340),
  ));

  p.finish(
    Some(acroform(&fields, true)),
    &out.join("acroform-basic.pdf"),
  );
}

fn inherited(out: &Path) {
  let mut p = Pdf::new(3);
  p.indirect_annots[1] = true;
  let mut fields = Vec::new();

  // person: /FT and /Ff on the parent, inherited by named kids first and last.
  let person = p.reserve();
  let first = p.merged(
    0,
    dictionary! { "T" => s("first"), "Parent" => Object::Reference(person), "V" => s("Ada") },
    rect(50, 700, 200, 720),
  );
  let last = p.merged(
    0,
    dictionary! { "T" => s("last"), "Parent" => Object::Reference(person), "V" => lopdf::text_string("Müller") },
    rect(250, 700, 400, 720),
  );
  p.put(
    person,
    dictionary! { "T" => s("person"), "FT" => n("Tx"), "Ff" => int(1), "Kids" => refs(&[first, last]) },
  );
  fields.push(person);

  // id: one field, two unnamed widgets on pages 2 and 3.
  let id_field = p.reserve();
  let mut id_kids = Vec::new();
  for page in [1usize, 2] {
    let mut kid = dictionary! { "Parent" => Object::Reference(id_field) };
    Pdf::widget(&mut kid, rect(50, 700, 200, 720));
    let kid_id = p.add(kid);
    p.page_annots[page].push(Object::Reference(kid_id));
    id_kids.push(kid_id);
  }
  p.put(
    id_field,
    dictionary! { "T" => s("id"), "FT" => n("Tx"), "V" => s("A-1"), "Kids" => refs(&id_kids) },
  );
  fields.push(id_field);

  fields.push(p.add(dictionary! { "T" => s("hidden"), "FT" => n("Tx"), "V" => s("secret") }));
  fields.push(p.text(0, "dup", s("one"), rect(50, 600, 200, 620)));
  fields.push(p.text(0, "dup", s("two"), rect(50, 560, 200, 580)));

  let strasse = Object::String(
    vec![0x53, 0x74, 0x72, 0x61, 0xDF, 0x65],
    StringFormat::Literal,
  );
  fields.push(p.merged(
    0,
    dictionary! { "T" => strasse, "FT" => n("Tx"), "V" => s("x") },
    rect(50, 500, 200, 520),
  ));
  fields.push(p.text(0, "amount", int(42), rect(50, 460, 200, 480)));
  fields.push(p.text(
    0,
    "badrect",
    s("b"),
    Object::Array(vec![int(0), int(0), int(100)]),
  ));

  // inlinekid: the widget is an inline dictionary inside /Kids; nothing in any /Annots.
  let mut inline_kid = Dictionary::new();
  Pdf::widget(&mut inline_kid, rect(50, 420, 200, 440));
  fields.push(p.add(dictionary! {
    "T" => s("inlinekid"), "FT" => n("Tx"),
    "Kids" => Object::Array(vec![Object::Dictionary(inline_kid)])
  }));

  fields.push(p.add(dictionary! {
    "T" => s("sigdata"), "FT" => n("Sig"),
    "V" => Object::Dictionary(dictionary! { "Type" => n("Sig"), "Filter" => n("Adobe.PPKLite") })
  }));

  let via_ref = p.add(s("via ref"));
  fields.push(p.text(
    0,
    "indirect",
    Object::Reference(via_ref),
    rect(50, 380, 200, 400),
  ));

  p.finish(
    Some(acroform(&fields, false)),
    &out.join("acroform-inherited.pdf"),
  );
}

fn cycle(out: &Path) {
  let mut p = Pdf::new(1);
  let a = p.reserve();
  let b = p.reserve();
  p.put(
    a,
    dictionary! { "T" => s("A"), "FT" => n("Tx"), "Kids" => refs(&[b]) },
  );
  let mut b_dict =
    dictionary! { "T" => s("B"), "Parent" => Object::Reference(a), "Kids" => refs(&[a]) };
  Pdf::widget(&mut b_dict, rect(50, 700, 200, 720));
  p.put(b, b_dict);
  p.page_annots[0].push(Object::Reference(b));
  p.finish(Some(acroform(&[a], false)), &out.join("acroform-cycle.pdf"));
}

fn orphans(out: &Path) {
  let mut p = Pdf::new(2);
  p.merged(
    0,
    dictionary! { "FT" => n("Tx"), "T" => s("ghost"), "V" => s("boo") },
    rect(50, 700, 200, 720),
  );
  let mut inline = dictionary! { "T" => s("inline") };
  Pdf::widget(&mut inline, rect(50, 700, 200, 720));
  p.page_annots[1].push(Object::Dictionary(inline));
  p.finish(None, &out.join("acroform-orphans.pdf"));
}

fn many(out: &Path) {
  let pages = 30usize;
  let mut p = Pdf::new(pages);
  let mut fields = Vec::new();
  for page in 0..pages {
    for i in 0..50i64 {
      let name = format!("f{}_{}", page + 1, i);
      let value = format!("v{}_{}", page + 1, i);
      fields.push(p.text(
        page,
        &name,
        s(&value),
        rect(50, 760 - 14 * i, 300, 770 - 14 * i),
      ));
    }
  }
  p.finish(
    Some(acroform(&fields, false)),
    &out.join("acroform-many.pdf"),
  );
}

/// Malformed-but-plausible structures the extractor must survive (final review of phase 1):
/// a `/V` that is a self-referencing array, a widget listed twice in `/Annots`, and a widget
/// whose field never made it into `/AcroForm /Fields` (name and value come through `/Parent`).
fn hostile(out: &Path) {
  let mut p = Pdf::new(1);
  let mut fields = Vec::new();

  let loop_arr = p.reserve();
  p.put(loop_arr, Object::Array(vec![Object::Reference(loop_arr)]));
  fields.push(p.text(
    0,
    "loopv",
    Object::Reference(loop_arr),
    rect(50, 700, 300, 720),
  ));

  let dup = p.text(0, "dupfield", s("twice"), rect(50, 660, 300, 680));
  p.page_annots[0].push(Object::Reference(dup));
  fields.push(dup);

  let orphan_parent = p.reserve();
  let mut orphan_widget = dictionary! { "Parent" => Object::Reference(orphan_parent) };
  Pdf::widget(&mut orphan_widget, rect(50, 620, 300, 640));
  let orphan_widget_id = p.add(orphan_widget);
  p.page_annots[0].push(Object::Reference(orphan_widget_id));
  p.put(
    orphan_parent,
    dictionary! {
      "T" => s("orphanParent"), "FT" => n("Tx"), "V" => s("val"), "Ff" => int(1),
      "Kids" => refs(&[orphan_widget_id])
    },
  );
  // deliberately NOT pushed to `fields`

  p.finish(
    Some(acroform(&fields, false)),
    &out.join("acroform-hostile.pdf"),
  );
}

/// Catalog `/AcroForm` is a dangling reference: no form can be read, and that must be reported.
fn dangling(out: &Path) {
  let p = Pdf::new(1);
  p.finish(
    Some(Object::Reference((999, 0))),
    &out.join("acroform-dangling.pdf"),
  );
}

// ── Embedded files ────────────────────────────────────────────────

impl Pdf {
  /// An embedded file stream plus its file specification; returns the spec id.
  fn embedded_file(
    &mut self,
    name: &str,
    mime: &str,
    data: &[u8],
    relationship: Option<&str>,
    description: Option<&str>,
    compress: bool,
  ) -> ObjectId {
    let mut stream = Stream::new(
      dictionary! {
        "Type" => "EmbeddedFile",
        "Subtype" => Object::Name(mime.as_bytes().to_vec()),
        "Params" => Object::Dictionary(dictionary! { "Size" => data.len() as i64 }),
      },
      data.to_vec(),
    );
    if compress {
      stream.compress().expect("compress embedded file");
    }
    let stream_id = self.doc.add_object(stream);
    let mut spec = dictionary! {
      "Type" => "Filespec",
      "F" => s(name),
      "UF" => lopdf::text_string(name),
      "EF" => Object::Dictionary(dictionary! {
        "F" => Object::Reference(stream_id),
        "UF" => Object::Reference(stream_id),
      }),
    };
    if let Some(r) = relationship {
      spec.set("AFRelationship", r);
    }
    if let Some(d) = description {
      spec.set("Desc", s(d));
    }
    self.doc.add_object(spec)
  }

  /// `/Names /EmbeddedFiles` as a flat leaf, plus optional catalog `/AF`.
  fn embedded_files_leaf(&mut self, entries: &[(&str, ObjectId)], af: &[ObjectId]) {
    let mut names = Vec::new();
    for (name, id) in entries {
      names.push(s(name));
      names.push(Object::Reference(*id));
    }
    let tree = Object::Dictionary(dictionary! { "Names" => names });
    self.catalog_extra.set(
      "Names",
      Object::Dictionary(dictionary! { "EmbeddedFiles" => tree }),
    );
    if !af.is_empty() {
      let af: Vec<Object> = af.iter().map(|id| Object::Reference(*id)).collect();
      self.catalog_extra.set("AF", Object::Array(af));
    }
  }

  /// `/Names /EmbeddedFiles` as a root with `/Kids`, one leaf per entry.
  fn embedded_files_kids(&mut self, entries: &[(&str, ObjectId)]) {
    let mut kids = Vec::new();
    for (name, id) in entries {
      let leaf = self.doc.add_object(dictionary! {
        "Limits" => vec![s(name), s(name)],
        "Names" => vec![s(name), Object::Reference(*id)],
      });
      kids.push(Object::Reference(leaf));
    }
    let root = self.doc.add_object(dictionary! { "Kids" => kids });
    self.catalog_extra.set(
      "Names",
      Object::Dictionary(dictionary! { "EmbeddedFiles" => Object::Reference(root) }),
    );
  }

  /// A `/FileAttachment` annotation on `page` pointing at `spec`.
  fn file_attachment_annot(&mut self, page: usize, spec: ObjectId) {
    let annot = self.doc.add_object(dictionary! {
      "Type" => "Annot",
      "Subtype" => "FileAttachment",
      "Rect" => rect(500, 700, 520, 720),
      "F" => 4,
      "Name" => "Paperclip",
      "FS" => Object::Reference(spec),
    });
    self.page_annots[page].push(Object::Reference(annot));
  }
}

/// Factur-X: CII XML as both `/AF` associated file and `/EmbeddedFiles` entry,
/// FlateDecode-compressed, `/AFRelationship /Alternative`.
fn facturx_minimal(out: &Path) {
  let xml = std::fs::read(out.join("cii-sample.xml")).expect("cii sample");
  let mut p = Pdf::new(1);
  let spec = p.embedded_file(
    "factur-x.xml",
    "text/xml",
    &xml,
    Some("Alternative"),
    Some("Factur-X invoice"),
    true,
  );
  p.embedded_files_leaf(&[("factur-x.xml", spec)], &[spec]);
  p.finish(None, &out.join("facturx-minimal.pdf"));
}

/// UBL invoice reachable only through a `/Kids` name tree, next to a plain text
/// attachment, plus a CSV attached as a page-2 `/FileAttachment` annotation.
fn ubl_attachments(out: &Path) {
  let xml = std::fs::read(out.join("ubl-sample.xml")).expect("ubl sample");
  let mut p = Pdf::new(2);
  let inv = p.embedded_file(
    "invoice.xml",
    "application/xml",
    &xml,
    Some("Data"),
    None,
    false,
  );
  let readme = p.embedded_file(
    "readme.txt",
    "text/plain",
    b"Supporting documents for UBL-77.\n",
    None,
    Some("Cover note"),
    false,
  );
  let csv = p.embedded_file(
    "notes.csv",
    "text/csv",
    b"line,qty\n1,10\n",
    None,
    None,
    false,
  );
  p.embedded_files_kids(&[("invoice.xml", inv), ("readme.txt", readme)]);
  p.file_attachment_annot(1, csv);
  p.finish(None, &out.join("ubl-attachments.pdf"));
}

/// An e-invoice attachment that is not well-formed XML.
fn einvoice_broken(out: &Path) {
  let mut p = Pdf::new(1);
  let spec = p.embedded_file(
    "factur-x.xml",
    "text/xml",
    b"<rsm:CrossIndustryInvoice><ram:ID>oops",
    Some("Alternative"),
    None,
    false,
  );
  p.embedded_files_leaf(&[("factur-x.xml", spec)], &[spec]);
  p.finish(None, &out.join("einvoice-broken.pdf"));
}

// ── Layout ───────────────────────────────────────────────────────

/// `__test__/layout.spec.ts` asserts the exact reading order and boxes below.
/// Page 1 is painted deliberately out of order (footer first, title second) with
/// Helvetica / Helvetica-Bold, a `TJ` array with kerning, `Tw` word spacing, a
/// form XObject placed through `/Matrix` + `cm`, and a 2×2 image XObject. Page 2
/// carries `/Rotate 90` and a CropBox smaller than its MediaBox.
fn layout_sample(out: &Path) {
  let mut p = Pdf::new(2);
  let helv = p.doc.add_object(dictionary! {
    "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding",
  });
  let helv_bold = p.doc.add_object(dictionary! {
    "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica-Bold", "Encoding" => "WinAnsiEncoding",
  });
  let form = p.doc.add_object(Stream::new(
    dictionary! {
      "Type" => "XObject", "Subtype" => "Form",
      "BBox" => vec![0.into(), 0.into(), 200.into(), 30.into()],
      "Matrix" => vec![1.into(), 0.into(), 0.into(), 1.into(), 20.into(), 10.into()],
      "Resources" => Object::Dictionary(dictionary! {
        "Font" => Object::Dictionary(dictionary! { "F1" => Object::Reference(helv) }),
      }),
    },
    b"BT /F1 11 Tf 0 0 Td (Inside a form XObject) Tj ET".to_vec(),
  ));
  let image = p.doc.add_object(Stream::new(
    dictionary! {
      "Type" => "XObject", "Subtype" => "Image", "Width" => 2, "Height" => 2,
      "ColorSpace" => "DeviceGray", "BitsPerComponent" => 8,
    },
    vec![0, 255, 255, 0],
  ));
  let resources = dictionary! {
    "Font" => Object::Dictionary(dictionary! {
      "F1" => Object::Reference(helv), "F2" => Object::Reference(helv_bold),
    }),
    "XObject" => Object::Dictionary(dictionary! {
      "Fx" => Object::Reference(form), "Im1" => Object::Reference(image),
    }),
  };
  let page1 = concat!(
    "BT /F1 9 Tf 50 30 Td (Page 1 of 2) Tj ET\n",
    "BT /F2 18 Tf 50 740 Td (Layout Sample) Tj ET\n",
    "BT /F2 10 Tf 50 600 Td (Item) Tj 250 0 Td (Qty) Tj 150 0 Td (Amount) Tj ET\n",
    "BT /F1 10 Tf 50 580 Td [(W) 40 (idget)] TJ 250 0 Td (2) Tj 150 0 Td (10,00) Tj ET\n",
    "BT /F1 10 Tf 50 560 Td (Gadget) Tj 250 0 Td (5) Tj 150 0 Td (25,00) Tj ET\n",
    "BT /F1 10 Tf 2 Tw 50 500 Td (Word spacing applies here) Tj ET\n",
    "q 1 0 0 1 300 400 cm /Fx Do Q\n",
    "q 100 0 0 50 400 650 cm /Im1 Do Q\n",
  );
  let page2 = "BT /F1 12 Tf 20 450 Td (Rotated page) Tj ET";
  for (i, content) in [page1, page2].into_iter().enumerate() {
    let cid = p
      .doc
      .add_object(Stream::new(dictionary! {}, content.as_bytes().to_vec()));
    p.set_page(i, "Contents", Object::Reference(cid));
    p.set_page(i, "Resources", Object::Dictionary(resources.clone()));
  }
  p.set_page(1, "Rotate", 90.into());
  p.set_page(
    1,
    "CropBox",
    Object::Array(vec![0.into(), 0.into(), 300.into(), 500.into()]),
  );
  p.finish(None, &out.join("layout-sample.pdf"));
}

// ── CCITT ────────────────────────────────────────────────────────

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

/// A one-page PDF whose only image is an 8×2 Group 4 fax stream, hand-coded:
/// row 1 `WWBBBWWW` (horizontal mode, white 2, black 3, then V0), row 2 all
/// white (pass mode, then V0), then EOFB. The same bits are decoded in the
/// `ccitt_tests` unit tests in `src/core/images.rs`.
fn ccitt_tiny(out: &Path) {
  let mut p = Pdf::new(1);
  let image = p.doc.add_object(Stream::new(
    dictionary! {
      "Type" => "XObject", "Subtype" => "Image", "Width" => 8, "Height" => 2,
      "ColorSpace" => "DeviceGray", "BitsPerComponent" => 1, "Filter" => "CCITTFaxDecode",
      "DecodeParms" => Object::Dictionary(dictionary! { "K" => -1, "Columns" => 8, "Rows" => 2 }),
    },
    bits("001 0111 10 1  0001 1  000000000001 000000000001"),
  ));
  let content = p.doc.add_object(Stream::new(
    dictionary! {},
    b"q 80 0 0 20 100 700 cm /Im1 Do Q".to_vec(),
  ));
  p.set_page(0, "Contents", Object::Reference(content));
  p.set_page(
    0,
    "Resources",
    Object::Dictionary(dictionary! {
      "XObject" => Object::Dictionary(dictionary! { "Im1" => Object::Reference(image) }),
    }),
  );
  p.finish(None, &out.join("ccitt-g4-tiny.pdf"));
}

/// A scanned page with a native stamp: hello-ocr's raster painted at its own aspect over most of a
/// short page, plus two native words above it. The cascade must OCR the scan and keep the stamp
/// (plan notes/2026-11-03/2155.md, Task 3).
fn stamped_scan(out: &Path) {
  let src = Document::load("ocr/__test__/fixtures/hello-ocr.pdf").expect("hello-ocr fixture");
  let image = src
    .objects
    .values()
    .find(|o| {
      matches!(o, Object::Stream(st) if st.dict.get(b"Subtype").ok().and_then(|v| v.as_name().ok()) == Some(b"Image"))
    })
    .cloned()
    .expect("hello-ocr carries one image XObject");
  let mut p = Pdf::new(1);
  let im = p.add(image);
  let helv = p.add(dictionary! {
    "Type" => n("Font"), "Subtype" => n("Type1"), "BaseFont" => n("Helvetica"), "Encoding" => n("WinAnsiEncoding")
  });
  let content = p.add(Stream::new(
    dictionary! {},
    // The stamp sits above the raster; "Hello" is painted natively over the raster's own "Hello"
    // (OCR reads it at about x 170-248, y 67-95) so the hybrid merge has a duplicate to drop.
    b"q 600 0 0 150 6 6 cm /Im1 Do Q\nBT /F1 10 Tf 40 164 Td (PO 4711) Tj ET\nBT /F1 25 Tf 180.8 74.5 Td (Hello) Tj ET".to_vec(),
  ));
  p.set_page(0, "MediaBox", rect(0, 0, 612, 180));
  p.set_page(0, "Contents", Object::Reference(content));
  p.set_page(
    0,
    "Resources",
    dictionary! {
      "Font" => dictionary! { "F1" => Object::Reference(helv) },
      "XObject" => dictionary! { "Im1" => Object::Reference(im) }
    }
    .into(),
  );
  p.finish(None, &out.join("stamped-scan.pdf"));
}

/// ASCII85 (PDF flavour: `<~` optional, `~>` terminator, `z` for four zero bytes).
fn ascii85_decode(data: &[u8]) -> Vec<u8> {
  let mut out = Vec::with_capacity(data.len() * 4 / 5);
  let mut group = [0u8; 5];
  let mut n = 0;
  let body = data.strip_prefix(b"<~").unwrap_or(data);
  for &b in body {
    match b {
      b'~' => break,
      b'z' if n == 0 => out.extend_from_slice(&[0, 0, 0, 0]),
      b'!'..=b'u' => {
        group[n] = b - b'!';
        n += 1;
        if n == 5 {
          let v = group.iter().fold(0u32, |acc, &d| acc * 85 + u32::from(d));
          out.extend_from_slice(&v.to_be_bytes());
          n = 0;
        }
      }
      _ => {}
    }
  }
  if n > 0 {
    for slot in group.iter_mut().skip(n) {
      *slot = 84;
    }
    let v = group.iter().fold(0u32, |acc, &d| acc * 85 + u32::from(d));
    out.extend_from_slice(&v.to_be_bytes()[..n - 1]);
  }
  out
}

/// The scanned sample page (`__test__/fixtures/ascii85-dct-scan.pdf`) as a gray raster: a full page
/// of text, which is what the orientation detector needs to be confident.
fn scanned_page_gray() -> GrayImage {
  let src = Document::load("__test__/fixtures/ascii85-dct-scan.pdf").expect("scan fixture");
  let stream = src
    .objects
    .values()
    .find_map(|o| match o {
      Object::Stream(st)
        if st.dict.get(b"Subtype").ok().and_then(|v| v.as_name().ok()) == Some(b"Image") =>
      {
        Some(st)
      }
      _ => None,
    })
    .expect("the scan carries one image XObject");
  let jpeg = ascii85_decode(&stream.content);
  image::load_from_memory(&jpeg)
    .expect("decode the JPEG raster")
    .to_luma8()
}

/// The scanned sample page turned a quarter turn clockwise and painted upright on a landscape page,
/// so its text reads sideways until the OCR tiers turn it back: 270° clockwise restores it
/// (plan notes/2026-11-03/2155.md, Task 4). The detector's confidence on this raster is ~10.
fn rotated_scan(out: &Path) {
  let turned = imageops::rotate90(&scanned_page_gray());
  let (w, h) = turned.dimensions();
  let mut jpeg = Vec::new();
  image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 70)
    .encode(turned.as_raw(), w, h, image::ExtendedColorType::L8)
    .expect("encode the turned raster");
  let image = Stream::new(
    dictionary! {
      "Type" => n("XObject"), "Subtype" => n("Image"), "Width" => int(w as i64), "Height" => int(h as i64),
      "ColorSpace" => n("DeviceGray"), "BitsPerComponent" => int(8), "Filter" => n("DCTDecode")
    },
    jpeg,
  );
  let mut p = Pdf::new(1);
  let im = p.add(image);
  let content = p.add(Stream::new(
    dictionary! {},
    b"q 792 0 0 560 0 26 cm /Im1 Do Q".to_vec(),
  ));
  p.set_page(0, "MediaBox", rect(0, 0, 792, 612));
  p.set_page(0, "Contents", Object::Reference(content));
  p.set_page(
    0,
    "Resources",
    dictionary! { "XObject" => dictionary! { "Im1" => Object::Reference(im) } }.into(),
  );
  p.finish(None, &out.join("rotated-scan.pdf"));
}

/// Three lines in three fonts: a barcode font (glyphs are bars), an anonymous `CIDFont+F1` (no weight
/// evidence) and Helvetica-Bold (plan notes/2026-11-03/2155.md, Task 5).
fn fonts_sample(out: &Path) {
  let mut p = Pdf::new(1);
  let barcode = p.add(dictionary! {
    "Type" => n("Font"), "Subtype" => n("TrueType"), "BaseFont" => n("IDAutomationHC39M"), "Encoding" => n("WinAnsiEncoding")
  });
  let anonymous = p.add(dictionary! {
    "Type" => n("Font"), "Subtype" => n("TrueType"), "BaseFont" => n("CIDFont+F1"), "Encoding" => n("WinAnsiEncoding")
  });
  let bold = p.add(dictionary! {
    "Type" => n("Font"), "Subtype" => n("Type1"), "BaseFont" => n("Helvetica-Bold"), "Encoding" => n("WinAnsiEncoding")
  });
  let content = p.add(Stream::new(
    dictionary! {},
    concat!(
      "BT /F1 14 Tf 50 700 Td (*12345*) Tj ET\n",
      "BT /F2 12 Tf 50 670 Td (Anonymous weight) Tj ET\n",
      "BT /F3 12 Tf 50 640 Td (Bold sample) Tj ET\n"
    )
    .as_bytes()
    .to_vec(),
  ));
  p.set_page(0, "Contents", Object::Reference(content));
  p.set_page(
    0,
    "Resources",
    dictionary! {
      "Font" => dictionary! {
        "F1" => Object::Reference(barcode), "F2" => Object::Reference(anonymous), "F3" => Object::Reference(bold)
      }
    }
    .into(),
  );
  p.finish(None, &out.join("fonts-sample.pdf"));
}

/// Three native words and no raster: a cover sheet, which must stay `Native` however sparse.
fn sparse_native(out: &Path) {
  let mut p = Pdf::new(1);
  let helv = p.add(dictionary! {
    "Type" => n("Font"), "Subtype" => n("Type1"), "BaseFont" => n("Helvetica"), "Encoding" => n("WinAnsiEncoding")
  });
  let content = p.add(Stream::new(
    dictionary! {},
    b"BT /F1 12 Tf 50 700 Td (Cover sheet only) Tj ET".to_vec(),
  ));
  p.set_page(0, "Contents", Object::Reference(content));
  p.set_page(
    0,
    "Resources",
    dictionary! { "Font" => dictionary! { "F1" => Object::Reference(helv) } }.into(),
  );
  p.finish(None, &out.join("sparse-native.pdf"));
}

fn main() {
  let out = PathBuf::from(
    std::env::args()
      .nth(1)
      .unwrap_or_else(|| "__test__/fixtures/forms".to_string()),
  );
  std::fs::create_dir_all(&out).expect("fixture dir");
  basic(&out);
  inherited(&out);
  cycle(&out);
  orphans(&out);
  many(&out);
  hostile(&out);
  dangling(&out);

  // The e-invoice fixtures live next to the forms directory and read their XML samples from it.
  let einvoice = out
    .parent()
    .unwrap_or_else(|| Path::new("__test__/fixtures"))
    .join("einvoice");
  std::fs::create_dir_all(&einvoice).expect("einvoice fixture dir");
  facturx_minimal(&einvoice);
  ubl_attachments(&einvoice);
  einvoice_broken(&einvoice);

  let layout = out
    .parent()
    .unwrap_or_else(|| Path::new("__test__/fixtures"))
    .join("layout");
  std::fs::create_dir_all(&layout).expect("layout fixture dir");
  layout_sample(&layout);
  sparse_native(&layout);
  fonts_sample(&layout);

  // OCR fixtures live with the OCR package's tests.
  let ocr_fixtures = Path::new("ocr/__test__/fixtures");
  std::fs::create_dir_all(ocr_fixtures).expect("ocr fixture dir");
  stamped_scan(ocr_fixtures);
  rotated_scan(ocr_fixtures);

  // The CCITT sample sits at the fixtures root, next to the other scan fixtures.
  let fixtures_root = out
    .parent()
    .unwrap_or_else(|| Path::new("__test__/fixtures"));
  ccitt_tiny(fixtures_root);

  // Spreadsheet fixtures: hand-written OOXML packages and the inputs detection must refuse.
  let spreadsheet = fixtures_root.join("spreadsheet");
  std::fs::create_dir_all(&spreadsheet).expect("spreadsheet fixture dir");
  spreadsheet_fixtures(&spreadsheet);

  // Invoice look-alikes for the layout track: invented text in the shape of the corpus's clusters.
  let invoices = fixtures_root.join("invoices");
  std::fs::create_dir_all(&invoices).expect("invoices fixture dir");
  invoice_fixtures(&invoices);
}

// ── Invoice look-alikes ───────────────────────────────────────────────────────

/// One text run: `(x, y, bold, size, text)` on a 612×792 page.
type Run = (f64, f64, bool, u32, &'static str);

fn pdf_text(run: &Run) -> String {
  let (x, y, bold, size, text) = run;
  let escaped = text
    .replace('\\', "\\\\")
    .replace('(', "\\(")
    .replace(')', "\\)");
  let font = if *bold { "F2" } else { "F1" };
  format!("BT /{font} {size} Tf {x} {y} Td ({escaped}) Tj ET\n")
}

fn invoice_page(out: &Path, name: &str, runs: &[Run]) {
  let mut p = Pdf::new(1);
  let helv = p.doc.add_object(dictionary! {
    "Type" => n("Font"), "Subtype" => n("Type1"), "BaseFont" => n("Helvetica"), "Encoding" => n("WinAnsiEncoding"),
  });
  let helv_bold = p.doc.add_object(dictionary! {
    "Type" => n("Font"), "Subtype" => n("Type1"), "BaseFont" => n("Helvetica-Bold"), "Encoding" => n("WinAnsiEncoding"),
  });
  let content: String = runs.iter().map(pdf_text).collect();
  let cid = p
    .doc
    .add_object(Stream::new(dictionary! {}, content.into_bytes()));
  p.set_page(0, "Contents", Object::Reference(cid));
  p.set_page(
    0,
    "Resources",
    Object::Dictionary(dictionary! {
      "Font" => Object::Dictionary(dictionary! {
        "F1" => Object::Reference(helv), "F2" => Object::Reference(helv_bold),
      }),
    }),
  );
  p.finish(None, &out.join(name));
}

fn invoice_fixtures(out: &Path) {
  // Cluster A: a US maintenance invoice with a labelled right column and a Bill To block.
  invoice_page(
    out,
    "cluster-a.pdf",
    &[
      (50.0, 760.0, true, 14, "ACME AVIATION SERVICES"),
      (50.0, 744.0, false, 10, "1200 Hangar Road"),
      (50.0, 732.0, false, 10, "Wichita, KS 67209"),
      (450.0, 760.0, true, 12, "INVOICE"),
      (380.0, 730.0, false, 10, "Invoice No: A-10045"),
      (380.0, 716.0, false, 10, "Invoice Date: 03/14/2024"),
      (380.0, 702.0, false, 10, "Customer PO: PO-778812"),
      (380.0, 688.0, false, 10, "Work Order: WO-5521"),
      (50.0, 690.0, false, 10, "Bill To:"),
      (50.0, 676.0, false, 10, "Skyline Charter LLC"),
      (50.0, 664.0, false, 10, "88 Terminal Drive"),
      (50.0, 652.0, false, 10, "Denver, CO 80249"),
      (50.0, 620.0, false, 10, "Aircraft Registration: N417SK"),
      (50.0, 606.0, false, 10, "Serial Number: 20311"),
      (400.0, 300.0, false, 10, "Subtotal"),
      (500.0, 300.0, false, 10, "$12,450.00"),
      (400.0, 286.0, false, 10, "Sales Tax"),
      (500.0, 286.0, false, 10, "$871.50"),
      (400.0, 270.0, true, 10, "Total Amount Due"),
      (500.0, 270.0, true, 10, "$13,321.50"),
    ],
  );
  // Cluster B: a German invoice, comma decimals, codes beside the amounts.
  invoice_page(
    out,
    "cluster-b.pdf",
    &[
      (50.0, 760.0, true, 14, "Mueller Flugtechnik GmbH"),
      (50.0, 744.0, false, 10, "Flughafenstrasse 12"),
      (50.0, 732.0, false, 10, "70629 Stuttgart"),
      (450.0, 760.0, true, 12, "RECHNUNG"),
      (380.0, 740.0, false, 10, "Rechnung Nr. 2015-0831"),
      (380.0, 726.0, false, 10, "Datum: 31.08.2015"),
      (380.0, 300.0, false, 10, "Zwischensumme"),
      (480.0, 300.0, false, 10, "EUR 1.234,56"),
      (380.0, 286.0, false, 10, "MwSt. 19%"),
      (480.0, 286.0, false, 10, "EUR 234,57"),
      (380.0, 270.0, true, 10, "Rechnungsbetrag"),
      (480.0, 270.0, true, 10, "EUR 1.469,13"),
    ],
  );
  // Cluster C: a bilingual Canadian invoice, space thousands, two tax lines, a code in brackets.
  invoice_page(
    out,
    "cluster-c.pdf",
    &[
      (50.0, 760.0, true, 14, "Nordique Aero Inc."),
      (50.0, 744.0, false, 10, "455 Rue de l'Aviation"),
      (50.0, 732.0, false, 10, "Montreal, QC H4T 1A1"),
      (400.0, 760.0, true, 12, "FACTURE / INVOICE"),
      (
        350.0,
        740.0,
        false,
        10,
        "No de facture / Invoice No.: F-2209",
      ),
      (350.0, 726.0, false, 10, "Date: 2022-09-15"),
      (350.0, 300.0, false, 10, "Sous-total / Subtotal"),
      (520.0, 300.0, false, 10, "4 500,00"),
      (350.0, 286.0, false, 10, "TPS / GST 5%"),
      (520.0, 286.0, false, 10, "225,00"),
      (350.0, 272.0, false, 10, "TVQ / QST 9.975%"),
      (520.0, 272.0, false, 10, "448,88"),
      (350.0, 256.0, true, 10, "Total (CAD)"),
      (520.0, 256.0, true, 10, "5 173,88"),
    ],
  );
  // A credit memo: the title names it and the total is negative.
  invoice_page(
    out,
    "credit-memo.pdf",
    &[
      (50.0, 760.0, true, 14, "ACME AVIATION SERVICES"),
      (50.0, 744.0, false, 10, "1200 Hangar Road"),
      (50.0, 732.0, false, 10, "Wichita, KS 67209"),
      (430.0, 760.0, true, 12, "CREDIT MEMO"),
      (380.0, 740.0, false, 10, "Credit Memo #: CM-2201"),
      (380.0, 726.0, false, 10, "Date: 04/02/2024"),
      (380.0, 712.0, false, 10, "Original Invoice: A-10045"),
      (400.0, 300.0, true, 10, "Total Credit"),
      (500.0, 300.0, true, 10, "-$1,200.00"),
    ],
  );
}

// ── Spreadsheet fixtures ──────────────────────────────────────────────────────

const WORKBOOK_MAIN: &str =
  "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml";
const MACRO_MAIN: &str = "application/vnd.ms-excel.sheet.macroEnabled.main+xml";
const SML: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const SML_STRICT: &str = "http://purl.oclc.org/ooxml/spreadsheetml/main";
const REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const OFFICE_DOC_REL: &str =
  "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
const WORKSHEET_REL: &str =
  "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet";

/// Write one zip package from `(path, xml)` parts with the given compression.
fn write_package(path: &Path, parts: &[(&str, String)], method: zip::CompressionMethod) {
  let file = std::fs::File::create(path).expect("package file");
  let mut writer = zip::ZipWriter::new(file);
  let options = zip::write::SimpleFileOptions::default().compression_method(method);
  for (name, xml) in parts {
    writer.start_file(*name, options).expect("start part");
    std::io::Write::write_all(&mut writer, xml.as_bytes()).expect("write part");
  }
  writer.finish().expect("finish package");
}

fn content_types(main_part: &str, main_type: &str, others: &[(&str, &str)]) -> String {
  let mut xml = String::from(
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\n<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\n<Default Extension=\"xml\" ContentType=\"application/xml\"/>\n",
  );
  xml.push_str(&format!(
    "<Override PartName=\"/{main_part}\" ContentType=\"{main_type}\"/>\n"
  ));
  for (part, kind) in others {
    xml.push_str(&format!(
      "<Override PartName=\"/{part}\" ContentType=\"{kind}\"/>\n"
    ));
  }
  xml.push_str("</Types>\n");
  xml
}

fn root_rels(main_part: &str) -> String {
  format!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"{REL_NS}\">\n<Relationship Id=\"rId1\" Type=\"{OFFICE_DOC_REL}\" Target=\"{main_part}\"/>\n</Relationships>\n"
  )
}

/// The parts of a one-sheet workbook with inline strings, a number, a boolean and an error, and
/// no shared strings or styles: the smallest package that is still a workbook.
fn minimal_workbook_parts(main_type: &str, sml: &str) -> Vec<(&'static str, String)> {
  vec![
    (
      "[Content_Types].xml",
      content_types(
        "xl/workbook.xml",
        main_type,
        &[(
          "xl/worksheets/sheet1.xml",
          "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml",
        )],
      ),
    ),
    ("_rels/.rels", root_rels("xl/workbook.xml")),
    (
      "xl/workbook.xml",
      format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<workbook xmlns=\"{sml}\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">\n<sheets><sheet name=\"Form\" sheetId=\"1\" r:id=\"rId1\"/></sheets>\n</workbook>\n"
      ),
    ),
    (
      "xl/_rels/workbook.xml.rels",
      format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"{REL_NS}\">\n<Relationship Id=\"rId1\" Type=\"{WORKSHEET_REL}\" Target=\"worksheets/sheet1.xml\"/>\n</Relationships>\n"
      ),
    ),
    (
      "xl/worksheets/sheet1.xml",
      format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<worksheet xmlns=\"{sml}\">\n<dimension ref=\"A1:B2\"/>\n<sheetData>\n<row r=\"1\"><c r=\"A1\" t=\"inlineStr\"><is><t>Total</t></is></c><c r=\"B1\"><v>427.5</v></c></row>\n<row r=\"2\"><c r=\"A2\" t=\"b\"><v>1</v></c><c r=\"B2\" t=\"e\"><v>#DIV/0!</v></c></row>\n</sheetData>\n</worksheet>\n"
      ),
    ),
  ]
}

fn spreadsheet_fixtures(out: &Path) {
  write_package(
    &out.join("minimal.xlsx"),
    &minimal_workbook_parts(WORKBOOK_MAIN, SML),
    zip::CompressionMethod::Deflated,
  );
  write_package(
    &out.join("macro.xlsm"),
    &minimal_workbook_parts(MACRO_MAIN, SML),
    zip::CompressionMethod::Deflated,
  );
  write_package(
    &out.join("strict.xlsx"),
    &minimal_workbook_parts(WORKBOOK_MAIN, SML_STRICT),
    zip::CompressionMethod::Deflated,
  );
  write_package(
    &out.join("not-a-workbook.docx"),
    &[
      (
        "[Content_Types].xml",
        content_types(
          "word/document.xml",
          "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
          &[],
        ),
      ),
      ("_rels/.rels", root_rels("word/document.xml")),
      (
        "word/document.xml",
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body/></w:document>\n".to_string(),
      ),
    ],
    zip::CompressionMethod::Stored,
  );
  let mut ole = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
  ole.resize(512, 0);
  std::fs::write(out.join("ole.bin"), ole).expect("ole fixture");
  write_package(
    &out.join("form.xlsx"),
    &form_workbook_parts(),
    zip::CompressionMethod::Deflated,
  );
  write_package(
    &out.join("date1904.xlsx"),
    &date1904_workbook_parts(),
    zip::CompressionMethod::Stored,
  );
}

const SHARED_STRINGS_REL: &str =
  "http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings";
const STYLES_REL: &str =
  "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";
const WORKSHEET_TYPE: &str =
  "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml";
const SHARED_STRINGS_TYPE: &str =
  "application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml";
const STYLES_TYPE: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml";
const XML_HEAD: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

/// A reimbursement-form look-alike: every edge case the cells and formatting specs assert.
fn form_workbook_parts() -> Vec<(&'static str, String)> {
  let shared = [
    "Maintenance Reimbursement Form",
    "Rate",
    "Hours",
    "Total",
    "Date",
    "Invoice",
    "000123",
    "Net",
    "Pct",
    "hidden value",
    "Line one&#10;Line two",
    "secret",
  ];
  let mut sst = format!(
    "{XML_HEAD}<sst xmlns=\"{SML}\" count=\"{n}\" uniqueCount=\"{n}\">\n",
    n = shared.len()
  );
  for text in shared {
    sst.push_str(&format!("<si><t xml:space=\"preserve\">{text}</t></si>\n"));
  }
  sst.push_str("</sst>\n");

  let alignment = "<alignment vertical=\"center\" wrapText=\"1\" indent=\"1\"/>";
  let styles = format!(
    "{XML_HEAD}<styleSheet xmlns=\"{SML}\">\n<numFmts count=\"5\">\n<numFmt numFmtId=\"164\" formatCode=\"#,##0.00&quot;/hr&quot;\"/>\n<numFmt numFmtId=\"165\" formatCode=\"0.0\"/>\n<numFmt numFmtId=\"166\" formatCode=\"mm/dd/yy\"/>\n<numFmt numFmtId=\"167\" formatCode=\"$#,##0.00;($#,##0.00)\"/>\n<numFmt numFmtId=\"168\" formatCode=\"0.00%\"/>\n</numFmts>\n<fonts count=\"2\">\n<font><sz val=\"11\"/><name val=\"Calibri\"/></font>\n<font><b/><sz val=\"10\"/><name val=\"Arial\"/></font>\n</fonts>\n<cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\"/></cellStyleXfs>\n<cellXfs count=\"7\">\n<xf numFmtId=\"0\" fontId=\"0\"/>\n<xf numFmtId=\"164\" fontId=\"1\" applyNumberFormat=\"1\" applyAlignment=\"1\">{alignment}</xf>\n<xf numFmtId=\"165\" fontId=\"1\">{alignment}</xf>\n<xf numFmtId=\"4\" fontId=\"1\">{alignment}</xf>\n<xf numFmtId=\"14\" fontId=\"1\">{alignment}</xf>\n<xf numFmtId=\"167\" fontId=\"1\">{alignment}</xf>\n<xf numFmtId=\"168\" fontId=\"1\">{alignment}</xf>\n</cellXfs>\n<cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles>\n</styleSheet>\n"
  );

  let sheet1 = format!(
    "{XML_HEAD}<worksheet xmlns=\"{SML}\">\n<dimension ref=\"A1:H30\"/>\n<sheetFormatPr defaultRowHeight=\"15\"/>\n<cols><col min=\"1\" max=\"1\" width=\"20\" customWidth=\"1\"/><col min=\"2\" max=\"2\" width=\"12\" customWidth=\"1\"/><col min=\"3\" max=\"3\" width=\"14\" customWidth=\"1\"/></cols>\n<sheetData>\n<row r=\"1\"><c r=\"A1\" s=\"1\" t=\"s\"><v>0</v></c></row>\n<row r=\"3\"><c r=\"B3\" t=\"s\"><v>1</v></c><c r=\"C3\" s=\"1\"><v>142.5</v></c></row>\n<row r=\"4\"><c r=\"B4\" t=\"s\"><v>2</v></c><c r=\"C4\" s=\"2\"><v>3</v></c></row>\n<row r=\"5\"><c r=\"B5\" t=\"s\"><v>3</v></c><c r=\"C5\" s=\"3\"><f>C3*C4</f><v>427.5</v></c></row>\n<row r=\"6\"><c r=\"B6\" t=\"s\"><v>4</v></c><c r=\"C6\" s=\"4\"><v>42247</v></c></row>\n<row r=\"7\"><c r=\"B7\" t=\"s\"><v>5</v></c><c r=\"C7\" t=\"s\"><v>6</v></c></row>\n<row r=\"8\"><c r=\"B8\" t=\"s\"><v>7</v></c><c r=\"C8\" s=\"5\"><v>-5</v></c></row>\n<row r=\"9\"><c r=\"B9\" t=\"s\"><v>8</v></c><c r=\"C9\" s=\"6\"><v>0.075</v></c></row>\n<row r=\"10\" hidden=\"1\"><c r=\"B10\" t=\"s\"><v>9</v></c></row>\n<row r=\"11\" ht=\"30\" customHeight=\"1\"><c r=\"A11\" s=\"1\" t=\"s\"><v>10</v></c></row>\n<row r=\"16\"><c r=\"B16\"><f t=\"shared\" ref=\"B16:B18\" si=\"0\">C3*2</f><v>285</v></c></row>\n<row r=\"17\"><c r=\"B17\" t=\"str\"><f t=\"shared\" si=\"0\"/><v></v></c></row>\n<row r=\"18\"><c r=\"B18\"><f t=\"shared\" si=\"0\"/></c></row>\n<row r=\"20\"><c r=\"D20\" t=\"e\"><v>#DIV/0!</v></c><c r=\"G20\" s=\"1\"/></row>\n<row r=\"21\"><c r=\"E21\" t=\"b\"><v>1</v></c></row>\n<row r=\"22\"><c r=\"C22\" s=\"4\"><v>60</v></c></row>\n<row r=\"30\"><c r=\"H30\" s=\"1\"/></row>\n</sheetData>\n<mergeCells count=\"2\"><mergeCell ref=\"A1:F1\"/><mergeCell ref=\"A11:A12\"/></mergeCells>\n</worksheet>\n"
  );
  let sheet2 = format!(
    "{XML_HEAD}<worksheet xmlns=\"{SML}\">\n<dimension ref=\"A1:A1\"/>\n<sheetData><row r=\"1\"><c r=\"A1\" t=\"s\"><v>11</v></c></row></sheetData>\n</worksheet>\n"
  );
  vec![
    (
      "[Content_Types].xml",
      content_types(
        "xl/workbook.xml",
        WORKBOOK_MAIN,
        &[
          ("xl/worksheets/sheet1.xml", WORKSHEET_TYPE),
          ("xl/worksheets/sheet2.xml", WORKSHEET_TYPE),
          ("xl/sharedStrings.xml", SHARED_STRINGS_TYPE),
          ("xl/styles.xml", STYLES_TYPE),
        ],
      ),
    ),
    ("_rels/.rels", root_rels("xl/workbook.xml")),
    (
      "xl/workbook.xml",
      format!(
        "{XML_HEAD}<workbook xmlns=\"{SML}\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">\n<sheets><sheet name=\"Form\" sheetId=\"1\" r:id=\"rId1\"/><sheet name=\"Notes\" sheetId=\"2\" state=\"hidden\" r:id=\"rId2\"/></sheets>\n<definedNames><definedName name=\"_xlnm.Print_Area\" localSheetId=\"0\">Form!$A$1:$F$12</definedName></definedNames>\n</workbook>\n"
      ),
    ),
    (
      "xl/_rels/workbook.xml.rels",
      format!(
        "{XML_HEAD}<Relationships xmlns=\"{REL_NS}\">\n<Relationship Id=\"rId1\" Type=\"{WORKSHEET_REL}\" Target=\"worksheets/sheet1.xml\"/>\n<Relationship Id=\"rId2\" Type=\"{WORKSHEET_REL}\" Target=\"worksheets/sheet2.xml\"/>\n<Relationship Id=\"rId3\" Type=\"{SHARED_STRINGS_REL}\" Target=\"sharedStrings.xml\"/>\n<Relationship Id=\"rId4\" Type=\"{STYLES_REL}\" Target=\"styles.xml\"/>\n</Relationships>\n"
      ),
    ),
    ("xl/sharedStrings.xml", sst),
    ("xl/styles.xml", styles),
    ("xl/worksheets/sheet1.xml", sheet1),
    ("xl/worksheets/sheet2.xml", sheet2),
  ]
}

/// The 1904 date system: serial 0 under a date format is 1904-01-01.
fn date1904_workbook_parts() -> Vec<(&'static str, String)> {
  vec![
    (
      "[Content_Types].xml",
      content_types(
        "xl/workbook.xml",
        WORKBOOK_MAIN,
        &[
          ("xl/worksheets/sheet1.xml", WORKSHEET_TYPE),
          ("xl/styles.xml", STYLES_TYPE),
        ],
      ),
    ),
    ("_rels/.rels", root_rels("xl/workbook.xml")),
    (
      "xl/workbook.xml",
      format!(
        "{XML_HEAD}<workbook xmlns=\"{SML}\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">\n<workbookPr date1904=\"1\"/>\n<sheets><sheet name=\"Dates\" sheetId=\"1\" r:id=\"rId1\"/></sheets>\n</workbook>\n"
      ),
    ),
    (
      "xl/_rels/workbook.xml.rels",
      format!(
        "{XML_HEAD}<Relationships xmlns=\"{REL_NS}\">\n<Relationship Id=\"rId1\" Type=\"{WORKSHEET_REL}\" Target=\"worksheets/sheet1.xml\"/>\n<Relationship Id=\"rId2\" Type=\"{STYLES_REL}\" Target=\"styles.xml\"/>\n</Relationships>\n"
      ),
    ),
    (
      "xl/styles.xml",
      format!(
        "{XML_HEAD}<styleSheet xmlns=\"{SML}\">\n<fonts count=\"1\"><font><sz val=\"11\"/><name val=\"Calibri\"/></font></fonts>\n<cellXfs count=\"2\"><xf numFmtId=\"0\" fontId=\"0\"/><xf numFmtId=\"14\" fontId=\"0\"/></cellXfs>\n</styleSheet>\n"
      ),
    ),
    (
      "xl/worksheets/sheet1.xml",
      format!(
        "{XML_HEAD}<worksheet xmlns=\"{SML}\">\n<dimension ref=\"A1:A1\"/>\n<sheetData><row r=\"1\"><c r=\"A1\" s=\"1\"><v>0</v></c></row></sheetData>\n</worksheet>\n"
      ),
    ),
  ]
}
