//! Generates the AcroForm test fixtures under `__test__/fixtures/forms/` with
//! lopdf alone, so every byte of every fixture is reproducible from this file.
//!
//! Run: `cargo run --example gen_fixtures`
//!
//! The ava tests in `__test__/forms.spec.ts` assert the exact contents listed
//! here; change both together or neither.

use lopdf::{
  Dictionary, Document, EncryptionState, EncryptionVersion, Object, ObjectId, Permissions, Stream,
  StringFormat, dictionary,
};
use std::fs;
use std::path::Path;

const OUT_DIR: &str = "__test__/fixtures/forms";

/// A document under construction: catalog, page tree, and page ids.
struct Pdf {
  doc: Document,
  catalog_id: ObjectId,
  page_ids: Vec<ObjectId>,
}

impl Pdf {
  fn new(pages: usize) -> Self {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let mut kids = Vec::new();
    let mut page_ids = Vec::new();
    for _ in 0..pages {
      let pid = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => Object::Reference(pages_id),
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
      });
      kids.push(Object::Reference(pid));
      page_ids.push(pid);
    }
    doc.objects.insert(
      pages_id,
      Object::Dictionary(dictionary! {
        "Type" => "Pages",
        "Kids" => kids,
        "Count" => pages as i64,
      }),
    );
    let catalog_id = doc.add_object(dictionary! {
      "Type" => "Catalog",
      "Pages" => Object::Reference(pages_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));
    Pdf {
      doc,
      catalog_id,
      page_ids,
    }
  }

  fn set(&mut self, id: ObjectId, key: &str, value: Object) {
    match self.doc.objects.get_mut(&id) {
      Some(Object::Dictionary(d)) => d.set(key, value),
      _ => panic!("{id:?} is not a dictionary"),
    }
  }

  /// Append widget references to a page's `/Annots` (created on first use).
  fn place(&mut self, page: usize, widgets: &[ObjectId]) {
    let pid = self.page_ids[page];
    let mut annots = match self.doc.objects.get(&pid) {
      Some(Object::Dictionary(d)) => match d.get(b"Annots") {
        Ok(Object::Array(a)) => a.clone(),
        _ => Vec::new(),
      },
      _ => Vec::new(),
    };
    annots.extend(widgets.iter().map(|w| Object::Reference(*w)));
    self.set(pid, "Annots", Object::Array(annots));
  }

  /// Make a page's `/Annots` an indirect reference to its array.
  fn indirect_annots(&mut self, page: usize) {
    let pid = self.page_ids[page];
    let annots = match self.doc.objects.get(&pid) {
      Some(Object::Dictionary(d)) => d.get(b"Annots").ok().cloned(),
      _ => None,
    };
    if let Some(arr) = annots {
      let id = self.doc.add_object(arr);
      self.set(pid, "Annots", Object::Reference(id));
    }
  }

  fn acroform(&mut self, fields: Vec<ObjectId>, need_appearances: bool) {
    let mut af = dictionary! {
      "Fields" => fields.into_iter().map(Object::Reference).collect::<Vec<_>>(),
      "DA" => Object::string_literal("/Helv 0 Tf 0 g"),
    };
    if need_appearances {
      af.set("NeedAppearances", true);
    }
    self.set(self.catalog_id, "AcroForm", Object::Dictionary(af));
  }

  /// An empty appearance stream, shared by every state that needs one.
  fn ap_stream(&mut self) -> ObjectId {
    self.doc.add_object(Stream::new(
      dictionary! {
        "Type" => "XObject",
        "Subtype" => "Form",
        "BBox" => vec![0.into(), 0.into(), 20.into(), 20.into()],
      },
      Vec::new(),
    ))
  }

  /// `/AP << /N << state stream ... >> >>` with the given state names.
  fn ap(&mut self, states: &[&str]) -> Object {
    let mut n = Dictionary::new();
    for s in states {
      let stream = self.ap_stream();
      n.set(*s, Object::Reference(stream));
    }
    Object::Dictionary(dictionary! { "N" => Object::Dictionary(n) })
  }

  fn save(&mut self, name: &str) {
    let path = Path::new(OUT_DIR).join(name);
    self.doc.save(&path).expect("save fixture");
    println!("wrote {}", path.display());
  }
}

fn rect(a: i64, b: i64, c: i64, d: i64) -> Object {
  Object::Array(vec![a.into(), b.into(), c.into(), d.into()])
}

fn widget(rect_obj: Object) -> Dictionary {
  dictionary! {
    "Type" => "Annot",
    "Subtype" => "Widget",
    "F" => 4,
    "Rect" => rect_obj,
  }
}

fn s(text: &str) -> Object {
  Object::string_literal(text)
}

/// `/T`-named field merged with its widget.
fn field(name: &str, ft: &str, rect_obj: Object) -> Dictionary {
  let mut d = widget(rect_obj);
  d.set("T", s(name));
  d.set("FT", ft);
  d
}

// ── Fixtures ──────────────────────────────────────────────────────

/// One page, nine logical fields, eleven widgets. Reading order:
/// name, notes, zip, agree, color x3, country, tags, sig, submit.
fn basic() {
  let mut p = Pdf::new(1);

  let mut name = field("name", "Tx", rect(50, 700, 300, 720));
  name.set("V", s("Jane Doe"));
  name.set("TU", s("Full name"));
  name.set("TM", s("fullName"));
  let name = p.doc.add_object(name);

  let mut notes = field("notes", "Tx", rect(50, 600, 500, 680));
  notes.set("Ff", 1 << 12);
  notes.set("MaxLen", 200);
  notes.set("V", lopdf::text_string("Line one\nLine two"));
  let notes = p.doc.add_object(notes);

  let mut zip = field("zip", "Tx", rect(50, 560, 150, 580));
  zip.set("Ff", 1 << 24);
  zip.set("MaxLen", 5);
  zip.set("V", s("60601"));
  let zip = p.doc.add_object(zip);

  let agree_ap = p.ap(&["Ja", "Off"]);
  let mut agree = field("agree", "Btn", rect(50, 520, 70, 540));
  agree.set("Ff", 1 << 1);
  agree.set("V", "Ja");
  agree.set("AS", "Ja");
  agree.set("AP", agree_ap);
  let agree = p.doc.add_object(agree);

  let color = p.doc.new_object_id();
  let mut color_kids = Vec::new();
  for (i, (x, as_state)) in [(50, "Off"), (100, "1"), (150, "Off")].iter().enumerate() {
    let ap = p.ap(&[&i.to_string(), "Off"]);
    let mut k = widget(rect(*x, 480, x + 20, 500));
    k.set("Parent", Object::Reference(color));
    k.set("AS", *as_state);
    k.set("AP", ap);
    color_kids.push(p.doc.add_object(k));
  }
  p.doc.objects.insert(
    color,
    Object::Dictionary(dictionary! {
      "T" => s("color"),
      "FT" => "Btn",
      "Ff" => (1 << 15) | (1 << 14),
      "Opt" => vec![s("Red"), s("Green"), s("Blue")],
      "V" => "1",
      "Kids" => color_kids.iter().map(|k| Object::Reference(*k)).collect::<Vec<_>>(),
    }),
  );

  let mut country = field("country", "Ch", rect(50, 440, 250, 460));
  country.set("Ff", (1 << 17) | (1 << 18));
  country.set(
    "Opt",
    vec![
      Object::Array(vec![s("US"), s("United States")]),
      Object::Array(vec![s("DE"), s("Germany")]),
    ],
  );
  country.set("V", s("US"));
  let country = p.doc.add_object(country);

  let mut tags = field("tags", "Ch", rect(50, 340, 250, 420));
  tags.set("Ff", 1 << 21);
  tags.set("Opt", vec![s("a"), s("b"), s("c")]);
  tags.set("V", vec![s("a"), s("c")]);
  tags.set("I", vec![0.into(), 2.into()]);
  let tags = p.doc.add_object(tags);

  let mut submit = field("submit", "Btn", rect(50, 300, 150, 320));
  submit.set("Ff", 1 << 16);
  submit.set("A", Object::Dictionary(dictionary! { "S" => "ResetForm" }));
  let submit = p.doc.add_object(submit);

  let sig = p
    .doc
    .add_object(field("sig", "Sig", rect(300, 300, 500, 340)));

  let mut placed = vec![name, notes, zip, agree];
  placed.extend(color_kids.iter().copied());
  placed.extend([country, tags, submit, sig]);
  p.place(0, &placed);
  p.acroform(
    vec![name, notes, zip, agree, color, country, tags, submit, sig],
    true,
  );
  p.save("acroform-basic.pdf");

  // RC4-128 with an empty user password (the "encrypted but opens without a prompt" case).
  // The standard security handler derives its key from the trailer /ID, so give the
  // document a fixed one first (deterministic bytes keep the fixture reproducible).
  let file_id = Object::String(
    (0x11u8..0x21u8).collect::<Vec<u8>>(),
    StringFormat::Hexadecimal,
  );
  p.doc.trailer.set("ID", vec![file_id.clone(), file_id]);
  let state = EncryptionState::try_from(EncryptionVersion::V2 {
    document: &p.doc,
    owner_password: "owner",
    user_password: "",
    key_length: 128,
    permissions: Permissions::all(),
  })
  .expect("rc4 encryption state");
  p.doc.encrypt(&state).expect("encrypt rc4");
  p.save("acroform-basic-rc4.pdf");
}

/// Three pages: inheritance, UTF-16 and PDFDocEncoding strings, indirect `/V`,
/// an indirect `/Annots`, duplicate names, a bad `/Rect`, a multi-page field,
/// and three unplaced fields (plain, inline-kid, signed signature).
fn inherited() {
  let mut p = Pdf::new(3);

  let person = p.doc.new_object_id();
  let mut first = widget(rect(50, 700, 200, 720));
  first.set("T", s("first"));
  first.set("V", s("Ada"));
  first.set("Parent", Object::Reference(person));
  let first = p.doc.add_object(first);
  let mut last = widget(rect(250, 700, 400, 720));
  last.set("T", s("last"));
  last.set("V", lopdf::text_string("Müller"));
  last.set("Parent", Object::Reference(person));
  let last = p.doc.add_object(last);
  p.doc.objects.insert(
    person,
    Object::Dictionary(dictionary! {
      "T" => s("person"),
      "FT" => "Tx",
      "Ff" => 1,
      "Kids" => vec![Object::Reference(first), Object::Reference(last)],
    }),
  );

  let id = p.doc.new_object_id();
  let mut id_kids = Vec::new();
  for _ in 0..2 {
    let mut k = widget(rect(50, 700, 200, 720));
    k.set("Parent", Object::Reference(id));
    id_kids.push(p.doc.add_object(k));
  }
  p.doc.objects.insert(
    id,
    Object::Dictionary(dictionary! {
      "T" => s("id"),
      "FT" => "Tx",
      "V" => s("A-1"),
      "Kids" => id_kids.iter().map(|k| Object::Reference(*k)).collect::<Vec<_>>(),
    }),
  );

  let hidden = p.doc.add_object(dictionary! {
    "T" => s("hidden"),
    "FT" => "Tx",
    "V" => s("secret"),
  });

  let mut dup1 = field("dup", "Tx", rect(50, 600, 200, 620));
  dup1.set("V", s("one"));
  let dup1 = p.doc.add_object(dup1);
  let mut dup2 = field("dup", "Tx", rect(50, 560, 200, 580));
  dup2.set("V", s("two"));
  let dup2 = p.doc.add_object(dup2);

  let mut strasse = widget(rect(50, 500, 200, 520));
  strasse.set(
    "T",
    Object::String(b"Stra\xDFe".to_vec(), StringFormat::Literal),
  );
  strasse.set("FT", "Tx");
  strasse.set("V", s("x"));
  let strasse = p.doc.add_object(strasse);

  let mut amount = field("amount", "Tx", rect(50, 460, 200, 480));
  amount.set("V", 42);
  let amount = p.doc.add_object(amount);

  let mut badrect = field(
    "badrect",
    "Tx",
    Object::Array(vec![0.into(), 0.into(), 100.into()]),
  );
  badrect.set("V", s("b"));
  let badrect = p.doc.add_object(badrect);

  let inlinekid = p.doc.add_object(dictionary! {
    "T" => s("inlinekid"),
    "FT" => "Tx",
    "Kids" => vec![Object::Dictionary(widget(rect(50, 420, 200, 440)))],
  });

  let via_ref = p.doc.add_object(s("via ref"));
  let mut indirect = field("indirect", "Tx", rect(50, 380, 200, 400));
  indirect.set("V", Object::Reference(via_ref));
  let indirect = p.doc.add_object(indirect);

  let sig_value = p.doc.add_object(dictionary! {
    "Type" => "Sig",
    "Filter" => "Adobe.PPKLite",
    "SubFilter" => "adbe.pkcs7.detached",
    "Contents" => Object::String(vec![0u8; 8], StringFormat::Hexadecimal),
  });
  let sealed = p.doc.add_object(dictionary! {
    "T" => s("sealed"),
    "FT" => "Sig",
    "V" => Object::Reference(sig_value),
  });

  p.place(
    0,
    &[first, last, dup1, dup2, strasse, amount, badrect, indirect],
  );
  p.place(1, &[id_kids[0]]);
  p.indirect_annots(1);
  p.place(2, &[id_kids[1]]);
  p.acroform(
    vec![
      person, id, hidden, dup1, dup2, strasse, amount, badrect, inlinekid, indirect, sealed,
    ],
    false,
  );
  p.save("acroform-inherited.pdf");
}

/// `/Fields [A]`, `A /Kids [B]`, `B /Kids [A]`; B is the placed widget.
fn cycle() {
  let mut p = Pdf::new(1);
  let a = p.doc.new_object_id();
  let b = p.doc.new_object_id();
  p.doc.objects.insert(
    a,
    Object::Dictionary(dictionary! {
      "T" => s("A"),
      "FT" => "Tx",
      "Kids" => vec![Object::Reference(b)],
    }),
  );
  let mut bd = widget(rect(50, 700, 200, 720));
  bd.set("T", s("B"));
  bd.set("Parent", Object::Reference(a));
  bd.set("Kids", vec![Object::Reference(a)]);
  p.doc.objects.insert(b, Object::Dictionary(bd));
  p.place(0, &[b]);
  p.acroform(vec![a], false);
  p.save("acroform-cycle.pdf");
}

/// No `/AcroForm`: one referenced widget on page 1, one inline widget on page 2.
fn orphans() {
  let mut p = Pdf::new(2);
  let mut ghost = field("ghost", "Tx", rect(50, 700, 200, 720));
  ghost.set("V", s("boo"));
  let ghost = p.doc.add_object(ghost);
  p.place(0, &[ghost]);
  let mut inline = widget(rect(50, 700, 200, 720));
  inline.set("T", s("inline"));
  let pid = p.page_ids[1];
  p.set(
    pid,
    "Annots",
    Object::Array(vec![Object::Dictionary(inline)]),
  );
  p.save("acroform-orphans.pdf");
}

/// 30 pages x 50 text fields, 1,500 placements.
fn many() {
  let mut p = Pdf::new(30);
  let mut fields = Vec::with_capacity(1500);
  for page in 0..30 {
    let mut on_page = Vec::with_capacity(50);
    for i in 0..50i64 {
      let f = field(
        &format!("f{}_{}", page + 1, i),
        "Tx",
        rect(50, 760 - 14 * i, 300, 770 - 14 * i),
      );
      on_page.push(p.doc.add_object(f));
    }
    p.place(page, &on_page);
    fields.extend(on_page);
  }
  p.acroform(fields, false);
  p.save("acroform-many.pdf");
}

// ── Embedded files ────────────────────────────────────────────────

const EINVOICE_DIR: &str = "__test__/fixtures/einvoice";

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
    self.set(
      self.catalog_id,
      "Names",
      Object::Dictionary(dictionary! { "EmbeddedFiles" => tree }),
    );
    if !af.is_empty() {
      let af: Vec<Object> = af.iter().map(|id| Object::Reference(*id)).collect();
      self.set(self.catalog_id, "AF", Object::Array(af));
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
    self.set(
      self.catalog_id,
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
    self.place(page, &[annot]);
  }
}

/// Factur-X: CII XML as both `/AF` associated file and `/EmbeddedFiles` entry,
/// FlateDecode-compressed, `/AFRelationship /Alternative`.
fn facturx_minimal() {
  let xml = std::fs::read(format!("{EINVOICE_DIR}/cii-sample.xml")).expect("cii sample");
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
  p.save("../einvoice/facturx-minimal.pdf");
}

/// UBL invoice reachable only through a `/Kids` name tree, next to a plain text
/// attachment, plus a CSV attached as a page-2 `/FileAttachment` annotation.
fn ubl_attachments() {
  let xml = std::fs::read(format!("{EINVOICE_DIR}/ubl-sample.xml")).expect("ubl sample");
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
  p.save("../einvoice/ubl-attachments.pdf");
}

/// An e-invoice attachment that is not well-formed XML.
fn einvoice_broken() {
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
  p.save("../einvoice/einvoice-broken.pdf");
}

// ── Layout ───────────────────────────────────────────────────────

const LAYOUT_DIR: &str = "__test__/fixtures/layout";

/// `__test__/layout.spec.ts` asserts the exact reading order and boxes below.
/// Page 1 is painted deliberately out of order (footer first, title second) with
/// Helvetica / Helvetica-Bold, a `TJ` array with kerning, `Tw` word spacing, a
/// form XObject placed through `/Matrix` + `cm`, and a 2×2 image XObject. Page 2
/// carries `/Rotate 90` and a CropBox smaller than its MediaBox.
fn layout_sample() {
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
    let pid = p.page_ids[i];
    p.set(pid, "Contents", Object::Reference(cid));
    p.set(pid, "Resources", Object::Dictionary(resources.clone()));
  }
  let p2 = p.page_ids[1];
  p.set(p2, "Rotate", 90.into());
  p.set(
    p2,
    "CropBox",
    Object::Array(vec![0.into(), 0.into(), 300.into(), 500.into()]),
  );
  p.save("../layout/layout-sample.pdf");
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
fn ccitt_tiny() {
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
  let pid = p.page_ids[0];
  p.set(pid, "Contents", Object::Reference(content));
  p.set(
    pid,
    "Resources",
    Object::Dictionary(dictionary! {
      "XObject" => Object::Dictionary(dictionary! { "Im1" => Object::Reference(image) }),
    }),
  );
  p.save("../ccitt-g4-tiny.pdf");
}

fn main() {
  fs::create_dir_all(OUT_DIR).expect("create fixture dir");
  basic();
  inherited();
  cycle();
  orphans();
  many();
  fs::create_dir_all(EINVOICE_DIR).expect("create einvoice fixture dir");
  facturx_minimal();
  ubl_attachments();
  einvoice_broken();
  fs::create_dir_all(LAYOUT_DIR).expect("create layout fixture dir");
  layout_sample();
  ccitt_tiny();
  println!(
    "note: acroform-basic-aes256.pdf is produced with qpdf (lopdf has no AES crypt filter):\n  \
     qpdf --encrypt \"\" \"\" 256 -- {OUT_DIR}/acroform-basic.pdf {OUT_DIR}/acroform-basic-aes256.pdf"
  );
}
