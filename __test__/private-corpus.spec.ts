// Local-only corpus run over real invoices in ocr/__test__/fixtures/private/.
// The directory is gitignored: the files, and anything derived from them, never
// reach the repository. The assertions here are structural; the per-file
// summary goes to ava's log and stays on the machine that ran it.

import { existsSync, readdirSync, readFileSync } from "fs";
import { join } from "path";

import test from "ava";

import { attachments, extractFormFieldsPerPage, extractInvoice, extractTextPerPage, pdfMetadata } from "../index";

const PRIVATE = join(import.meta.dirname, "..", "ocr", "__test__", "fixtures", "private");

// One level of subdirectories is allowed (the corpus may be grouped by id prefix).
const files = existsSync(PRIVATE)
  ? readdirSync(PRIVATE, { recursive: true })
      .map(String)
      .filter((n) => /\.pdf$/i.test(n))
      .sort()
  : [];

(files.length > 0 ? test : test.skip)(
  "private corpus: every invoice parses and every extractor returns a well-formed result",
  (t) => {
    const widths = { name: Math.max(...files.map((n) => n.length), 4) };
    t.log(`${files.length} private invoice(s)`);
    t.log(`${"file".padEnd(widths.name)}  pages  nativeChars  form  attach  invoice`);
    for (const name of files) {
      const buf = readFileSync(join(PRIVATE, name));
      const meta = pdfMetadata(buf);
      const text = extractTextPerPage(buf);
      const fields = extractFormFieldsPerPage(buf);
      const att = attachments(buf);
      const inv = extractInvoice(buf);

      t.true(meta.pageCount >= 1, `${name}: page count`);
      t.is(text.length, meta.pageCount, `${name}: one text entry per page`);
      for (const f of fields) {
        t.true(f.page >= 1 && f.page <= meta.pageCount, `${name}: form field page in range`);
      }
      if (meta.form) {
        t.is(meta.form.widgetCount, fields.length, `${name}: widgetCount matches placements`);
      } else {
        t.is(fields.length, 0, `${name}: no form info means no placements`);
      }
      for (const a of att) {
        t.is(a.size, a.data.length, `${name}: attachment size matches payload`);
      }
      t.true(inv.confidence >= 0 && inv.confidence <= 1, `${name}: confidence in range`);
      if (inv.source === "None") {
        t.true(inv.warnings.length >= 1, `${name}: a result without a source explains why`);
      } else {
        t.truthy(inv.fields.invoiceId ?? inv.fields.invoiceTotal, `${name}: an extracted invoice has an id or a total`);
      }

      const nativeChars = text.reduce((n, p) => n + p.text.replace(/\s+/g, "").length, 0);
      const formCol = meta.form ? `${meta.form.fieldCount}/${meta.form.widgetCount}` : "-";
      const invCol = inv.source === "EmbeddedXml" ? `${inv.source} ${inv.embeddedXml?.standard}` : inv.source;
      t.log(
        `${name.padEnd(widths.name)}  ${String(meta.pageCount).padStart(5)}  ${String(nativeChars).padStart(11)}  ${formCol.padStart(4)}  ${String(att.length).padStart(6)}  ${invCol}`
      );
    }
  }
);
