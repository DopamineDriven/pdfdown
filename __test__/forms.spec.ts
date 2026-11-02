import { readFileSync } from "fs";
import { join } from "path";

import test from "ava";

import {
  extractAnnotationsPerPage,
  extractFormFieldsPerPage,
  extractFormFieldsPerPageAsync,
  pdfDocument,
  pdfDocumentAsync,
  pdfMetadata,
  PdfDown
} from "../index";
import type { FormFieldType, PageFormField } from "../index";

const FIXTURES = join(import.meta.dirname, "fixtures", "forms");
const fixture = (name: string) => readFileSync(join(FIXTURES, name));
const scan = () => readFileSync(join(import.meta.dirname, "fixtures", "ascii85-dct-scan.pdf"));
const whmcs = () =>
  readFileSync(join(import.meta.dirname, "fixtures", "invoices", "Downloadable-PDF-Invoices-Add-On-Samples.pdf"));

type Variant<K extends FormFieldType> = Extract<PageFormField, { type: K }>;

/** The single field named `name`, narrowed to `type`; throws (failing the test) otherwise. */
function one<K extends FormFieldType>(fields: PageFormField[], name: string, type: K): Variant<K> {
  const matches = fields.filter((f) => f.name === name);
  if (matches.length !== 1) throw new Error(`expected exactly one field named ${name}, got ${matches.length}`);
  const f = matches[0];
  if (f.type !== type) throw new Error(`expected ${name} to be ${type}, got ${f.type}`);
  return f as Variant<K>;
}

const BASIC_ORDER = ["name", "notes", "zip", "agree", "color", "color", "color", "country", "tags", "sig", "submit"];

test("basic: 11 placements, 9 logical fields, reading order", (t) => {
  const fields = extractFormFieldsPerPage(fixture("acroform-basic.pdf"));
  t.is(fields.length, 11);
  t.deepEqual(
    fields.map((f) => f.name),
    BASIC_ORDER
  );
  t.true(fields.every((f) => f.page === 1));
  t.is(new Set(fields.map((f) => f.fieldObjectId)).size, 9);
});

test("basic: text fields carry flags, limits and strings", (t) => {
  const fields = extractFormFieldsPerPage(fixture("acroform-basic.pdf"));
  const name = one(fields, "name", "Text");
  t.is(name.value, "Jane Doe");
  t.is(name.tooltip, "Full name");
  t.is(name.mappingName, "fullName");
  t.is(name.maxLen, undefined);
  t.false(name.multiline);
  const notes = one(fields, "notes", "Text");
  t.true(notes.multiline);
  t.is(notes.maxLen, 200);
  t.is(notes.value, "Line one\nLine two");
  const zip = one(fields, "zip", "Text");
  t.true(zip.comb);
  t.is(zip.maxLen, 5);
  t.is(zip.value, "60601");
});

test("basic: checkbox reports state name, export value and /AS state", (t) => {
  const agree = one(extractFormFieldsPerPage(fixture("acroform-basic.pdf")), "agree", "Checkbox");
  t.is(agree.onState, "Ja");
  t.is(agree.exportValue, "Ja");
  t.true(agree.checked);
  t.true(agree.required);
  t.is(agree.value, "Ja");
});

test("basic: radio keeps /AP state names and /Opt export values apart", (t) => {
  const radios = extractFormFieldsPerPage(fixture("acroform-basic.pdf")).filter((f) => f.name === "color");
  t.is(radios.length, 3);
  t.is(new Set(radios.map((f) => f.fieldObjectId)).size, 1);
  t.deepEqual(
    radios.map((f) => f.widgetIndex),
    [0, 1, 2]
  );
  t.true(radios.every((f) => f.widgetCount === 3));
  const parts = radios.map((f) => {
    if (f.type !== "Radio") throw new Error(`expected Radio, got ${f.type}`);
    return f;
  });
  t.deepEqual(
    parts.map((f) => f.onState),
    ["0", "1", "2"]
  );
  t.deepEqual(
    parts.map((f) => f.exportValue),
    ["Red", "Green", "Blue"]
  );
  t.deepEqual(
    parts.map((f) => f.selected),
    [false, true, false]
  );
  t.true(parts.every((f) => f.value === "Green"));
});

test("basic: combo, list, push button, signature", (t) => {
  const fields = extractFormFieldsPerPage(fixture("acroform-basic.pdf"));
  const country = one(fields, "country", "ComboBox");
  t.true(country.editable);
  t.deepEqual(country.options, [
    { exportValue: "US", displayValue: "United States" },
    { exportValue: "DE", displayValue: "Germany" }
  ]);
  t.is(country.value, "US");
  const tags = one(fields, "tags", "ListBox");
  t.true(tags.multiSelect);
  t.deepEqual(tags.values, ["a", "c"]);
  t.is(tags.value, "a");
  const submit = one(fields, "submit", "PushButton");
  t.true(submit.hasActions);
  const sig = one(fields, "sig", "Signature");
  t.false(sig.signed);
  t.is(sig.value, undefined);
});

test("inherited: dotted names, inherited flags, UTF-16, PDFDocEncoding, indirect /V", (t) => {
  const fields = extractFormFieldsPerPage(fixture("acroform-inherited.pdf"));
  const first = one(fields, "person.first", "Text");
  const last = one(fields, "person.last", "Text");
  t.true(first.readOnly);
  t.true(last.readOnly);
  t.is(first.value, "Ada");
  t.is(last.value, "Müller");
  t.is(one(fields, "Straße", "Text").value, "x");
  t.is(one(fields, "amount", "Text").value, "42");
  t.is(one(fields, "badrect", "Text").rect.length, 3);
  t.is(one(fields, "indirect", "Text").value, "via ref");
});

test("inherited: multi-page placement and duplicate names", (t) => {
  const fields = extractFormFieldsPerPage(fixture("acroform-inherited.pdf"));
  t.is(fields.length, 10);
  const id = fields.filter((f) => f.name === "id");
  t.deepEqual(
    id.map((f) => f.page),
    [2, 3]
  );
  t.deepEqual(
    id.map((f) => f.widgetIndex),
    [0, 1]
  );
  t.true(id.every((f) => f.widgetCount === 2));
  t.is(new Set(id.map((f) => f.fieldObjectId)).size, 1);
  const dup = fields.filter((f) => f.name === "dup");
  t.is(dup.length, 2);
  t.not(dup[0].fieldObjectId, dup[1].fieldObjectId);
  t.deepEqual(
    dup.map((f) => f.value),
    ["one", "two"]
  );
});

test("cycle: terminates with the single placed field", (t) => {
  const fields = extractFormFieldsPerPage(fixture("acroform-cycle.pdf"));
  t.is(fields.length, 1);
  t.is(fields[0].name, "A.B");
});

test("orphans: widgets without /AcroForm are Unknown; annotations keep them", (t) => {
  const buf = fixture("acroform-orphans.pdf");
  const fields = extractFormFieldsPerPage(buf);
  t.is(fields.length, 2);
  t.true(fields.every((f) => f.type === "Unknown"));
  t.deepEqual(
    fields.map((f) => f.name),
    ["ghost", "inline"]
  );
  t.is(fields[0].value, "boo");
  t.is(fields[1].objectId, "page 2 annot 0");
  t.is(extractAnnotationsPerPage(buf).filter((a) => a.subtype === "Widget").length, 2);
});

test("no form: existing fixture yields an empty array", (t) => {
  const scan = readFileSync(join(import.meta.dirname, "fixtures", "ascii85-dct-scan.pdf"));
  t.deepEqual(extractFormFieldsPerPage(scan), []);
});

test("many: 1,500 placements on 30 pages in page order", (t) => {
  const buf = fixture("acroform-many.pdf");
  const started = performance.now();
  const fields = extractFormFieldsPerPage(buf);
  t.log(`extractFormFieldsPerPage(acroform-many.pdf): ${(performance.now() - started).toFixed(1)} ms`);
  t.is(fields.length, 1500);
  t.is(fields[0].page, 1);
  t.is(fields[fields.length - 1].page, 30);
  for (let i = 1; i < fields.length; i++) t.true(fields[i].page >= fields[i - 1].page);
});

for (const [label, file] of [
  ["RC4-128", "acroform-basic-rc4.pdf"],
  ["AES-256", "acroform-basic-aes256.pdf"]
] as const) {
  test(`encrypted ${label}, empty user password: same fields as plaintext`, (t) => {
    const fields = extractFormFieldsPerPage(fixture(file));
    t.deepEqual(
      fields.map((f) => f.name),
      BASIC_ORDER
    );
    t.is(one(fields, "name", "Text").value, "Jane Doe");
  });
}

test("sync, async and class agree", async (t) => {
  for (const file of ["acroform-basic.pdf", "acroform-inherited.pdf"]) {
    const buf = fixture(file);
    const sync = extractFormFieldsPerPage(buf);
    t.deepEqual(await extractFormFieldsPerPageAsync(buf), sync);
    const pdf = new PdfDown(buf);
    t.deepEqual(pdf.formFieldsPerPage(), sync);
    t.deepEqual(await pdf.formFieldsPerPageAsync(), sync);
  }
});

test("pdfMetadata: form info present on forms, absent otherwise", (t) => {
  const basic = pdfMetadata(fixture("acroform-basic.pdf")).form;
  if (!basic) throw new Error("basic fixture must report form info");
  t.true(basic.hasAcroForm);
  t.true(basic.needAppearances);
  t.is(basic.fieldCount, 9);
  t.is(basic.widgetCount, 11);
  t.deepEqual(basic.unplacedFields, []);
  t.false(basic.truncated);
  t.deepEqual(basic.warnings, []);

  const inherited = pdfMetadata(fixture("acroform-inherited.pdf")).form;
  if (!inherited) throw new Error("inherited fixture must report form info");
  t.is(inherited.fieldCount, 12);
  t.is(inherited.widgetCount, 10);
  t.deepEqual(
    inherited.unplacedFields.map((u) => u.name),
    ["hidden", "inlinekid", "sigdata"]
  );
  const [hidden, inlinekid, sigdata] = inherited.unplacedFields;
  t.is(hidden.type, "Text");
  t.is(hidden.value, "secret");
  t.false(hidden.signed);
  t.regex(inlinekid.fieldObjectId, /^\d+ \d+ obj$/);
  t.is(sigdata.type, "Signature");
  t.true(sigdata.signed);

  const orphans = pdfMetadata(fixture("acroform-orphans.pdf")).form;
  if (!orphans) throw new Error("orphan widgets must report form info");
  t.false(orphans.hasAcroForm);
  t.is(orphans.fieldCount, 0);
  t.is(orphans.widgetCount, 2);

  t.is(pdfMetadata(scan()).form, undefined);
});

test("real world: WHMCS invoice with an empty /AcroForm", (t) => {
  const doc = pdfDocument(whmcs());
  if (!doc.form) throw new Error("an /AcroForm dictionary must report form info even with no fields");
  t.true(doc.form.hasAcroForm);
  t.is(doc.form.fieldCount, 0);
  t.is(doc.form.widgetCount, 0);
  t.deepEqual(doc.formFields, []);
  t.deepEqual(doc.formFieldPages, []);
  t.is(doc.totalFormFields, 0);
});

test("pdfDocument: aggregates mirror images and annotations", (t) => {
  const buf = fixture("acroform-inherited.pdf");
  const doc = pdfDocument(buf);
  t.is(doc.totalFormFields, 10);
  t.is(doc.formFields.length, 10);
  t.deepEqual(doc.formFieldPages, [1, 2, 3]);
  t.deepEqual(doc.form, pdfMetadata(buf).form);
  t.deepEqual(doc.formFields, extractFormFieldsPerPage(buf));

  const none = pdfDocument(scan());
  t.deepEqual(none.formFields, []);
  t.is(none.totalFormFields, 0);
  t.deepEqual(none.formFieldPages, []);
  t.is(none.form, undefined);
});

test("pdfDocumentAsync and PdfDown.document agree on form data", async (t) => {
  const buf = fixture("acroform-basic.pdf");
  const sync = pdfDocument(buf);
  if (!sync.form) throw new Error("sync pdfDocument must carry form info");
  t.is(sync.formFields.length, 11);
  const viaAsync = await pdfDocumentAsync(buf);
  t.deepEqual(viaAsync.form, sync.form);
  t.deepEqual(viaAsync.formFields, sync.formFields);
  t.is(viaAsync.totalFormFields, sync.totalFormFields);
  const pdf = new PdfDown(buf);
  t.deepEqual(pdf.document().formFields, sync.formFields);
  t.deepEqual((await pdf.documentAsync()).form, sync.form);
  t.deepEqual(pdf.metadata().form, sync.form);
  t.deepEqual((await pdf.metadataAsync()).form, sync.form);
});
