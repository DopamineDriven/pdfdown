//! Generates the AcroForm test fixtures under `__test__/fixtures/forms/` (plan: notes/phase-1.md,
//! Task 5) and the e-invoice attachment fixtures under `__test__/fixtures/einvoice/`. Deterministic, pure lopdf, our own code. Re-run with `cargo run --example gen_fixtures`
//! and commit the output together with any test that depends on it.

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
    }
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
      self.put(
        pid,
        dictionary! {
          "Type" => n("Page"),
          "Parent" => Object::Reference(pages_id),
          "MediaBox" => rect(0, 0, 612, 792),
          "Annots" => annots
        },
      );
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
    catalog.extend(&extra);
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
}
