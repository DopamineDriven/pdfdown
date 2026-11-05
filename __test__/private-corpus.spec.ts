// Local-only corpus run over real invoices in ocr/__test__/fixtures/private/.
// The directory is gitignored: the files, and anything derived from them, never
// reach the repository. The assertions here are structural; the per-file
// summary goes to ava's log, and the detailed results are written next to the
// PDFs as report.json and report.ts (both inside the ignored directory). The
// corpus's workbooks get the same treatment with aggregates only in the log.

import { existsSync, readFileSync, writeFileSync } from "fs";
import { join, relative } from "path";

import test from "ava";

import {
  PdfDown,
  attachments,
  detectFormat,
  extractFormFieldsPerPage,
  extractInvoice,
  extractTextPerPage,
  layoutPerPage,
  pdfMetadata
} from "../index";
import type { Attachment, FormInfo, InvoiceResult, PageFormField, PageText, PdfMeta, TextSource } from "../index";
import { discoverCorpus } from "../scripts/lib/corpus";

/** One private invoice's results. Attachment payloads are omitted; everything else is verbatim. */
interface PrivateCorpusEntry {
  id: string;
  file: string;
  bytes: number;
  meta: Omit<PdfMeta, "form">;
  form: FormInfo | null;
  formFields: Array<PageFormField>;
  attachments: Array<Omit<Attachment, "data">>;
  text: Array<PageText>;
  nativeChars: number;
  invoice: InvoiceResult;
  layout: Array<PrivateCorpusPageLayout>;
}

/** Positioned-text summary of one page: counts plus the line texts in reading order. */
interface PrivateCorpusPageLayout {
  page: number;
  source: TextSource;
  width: number;
  height: number;
  rotate: number;
  words: number;
  lines: number;
  images: number;
  fonts: Array<string>;
  warnings: Array<string>;
  lineText: Array<string>;
}

interface PrivateCorpusReport {
  generatedAt: string;
  files: number;
  entries: Array<PrivateCorpusEntry>;
}

const PRIVATE = join(import.meta.dirname, "..", "ocr", "__test__", "fixtures", "private");

// One level of subdirectories is allowed (the corpus may be grouped by id prefix). The document
// store names everything .pdf, so files are sorted by what they are, not by their extension.
const named = existsSync(PRIVATE) ? discoverCorpus(PRIVATE).filter((f) => f.kind === "pdf") : [];
const sniffed = named.map((f) => ({ file: f, detection: detectFormat(readFileSync(f.path)) }));
const corpus = sniffed.filter((s) => s.detection.format === "Pdf").map((s) => s.file);
const notPdf = sniffed.filter((s) => s.detection.format !== "Pdf");
const files = corpus.map((f) => relative(PRIVATE, f.path));

(files.length > 0 ? test : test.skip)(
  "private corpus: every invoice parses and every extractor returns a well-formed result",
  (t) => {
    t.timeout(10 * 60 * 1000, "every invoice in the private corpus through every extractor");
    const widths = { name: Math.max(...files.map((n) => n.length), 4) };
    const entries: Array<PrivateCorpusEntry> = [];
    t.log(`${files.length} private invoice(s)`);
    if (notPdf.length > 0) {
      const kinds = new Map<string, number>();
      for (const s of notPdf)
        kinds.set(s.detection.detail ?? "unknown", (kinds.get(s.detection.detail ?? "unknown") ?? 0) + 1);
      t.log(`${notPdf.length} file(s) named .pdf are not PDFs: ${[...kinds].map(([k, n]) => `${k} ${n}`).join(", ")}`);
    }
    t.log(`${"file".padEnd(widths.name)}  pages  nativeChars  words  form  attach  invoice`);
    for (const file of corpus) {
      const name = relative(PRIVATE, file.path);
      const buf = readFileSync(file.path);
      const meta = pdfMetadata(buf);
      const text = extractTextPerPage(buf);
      const fields = extractFormFieldsPerPage(buf);
      const att = attachments(buf);
      const inv = extractInvoice(buf);
      const layout = layoutPerPage(buf);

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

      t.is(layout.length, meta.pageCount, `${name}: one layout per page`);
      for (const p of layout) {
        t.is(p.source, "Native", `${name} p${p.page}: native layout`);
        let next = 0;
        for (const [i, line] of p.lines.entries()) {
          t.is(line.firstWord, next, `${name} p${p.page} line ${i}: contiguous firstWord`);
          const slice = p.words.slice(line.firstWord, line.firstWord + line.wordCount);
          t.is(slice.length, line.wordCount, `${name} p${p.page} line ${i}: wordCount in range`);
          t.true(
            slice.every((w) => w.line === i),
            `${name} p${p.page} line ${i}: words point back`
          );
          if (i > 0) {
            t.true(line.y1 <= p.lines[i - 1].y1 + 0.5, `${name} p${p.page} line ${i}: top-to-bottom`);
          }
          next += line.wordCount;
        }
        t.is(next, p.words.length, `${name} p${p.page}: lines cover every word`);
      }
      const layoutWords = layout.reduce((n, p) => n + p.words.length, 0);

      const nativeChars = text.reduce((n, p) => n + p.text.replace(/\s+/g, "").length, 0);
      const { form, ...metaRest } = meta;
      entries.push({
        id: file.id,
        file: name,
        bytes: buf.length,
        meta: metaRest,
        form: form ?? null,
        formFields: fields,
        attachments: att.map(({ data: _data, ...rest }) => rest),
        text,
        nativeChars,
        invoice: inv,
        layout: layout.map((p) => ({
          page: p.page,
          source: p.source,
          width: p.width,
          height: p.height,
          rotate: p.rotate,
          words: p.words.length,
          lines: p.lines.length,
          images: p.images.length,
          fonts: [...new Set(p.words.map((w) => `${w.fontName ?? "?"}${w.bold ? " (bold)" : ""}`))],
          warnings: p.warnings,
          lineText: p.lines.map((l) => l.text)
        }))
      });
      const formCol = meta.form ? `${meta.form.fieldCount}/${meta.form.widgetCount}` : "-";
      const invCol = inv.source === "EmbeddedXml" ? `${inv.source} ${inv.embeddedXml?.standard}` : inv.source;
      t.log(
        `${name.padEnd(widths.name)}  ${String(meta.pageCount).padStart(5)}  ${String(nativeChars).padStart(11)}  ${String(layoutWords).padStart(5)}  ${formCol.padStart(4)}  ${String(att.length).padStart(6)}  ${invCol}`
      );
    }

    const report: PrivateCorpusReport = {
      generatedAt: new Date().toISOString(),
      files: entries.length,
      entries
    };
    const json = JSON.stringify(report, null, 2);
    writeFileSync(join(PRIVATE, "report.json"), `${json}\n`);
    writeFileSync(
      join(PRIVATE, "report.ts"),
      [
        "// Generated by __test__/private-corpus.spec.ts. Lives in a gitignored directory; do not commit.",
        'import type { Attachment, FormInfo, InvoiceResult, PageFormField, PageText, PdfMeta, TextSource } from "../../../../index";',
        "",
        "export interface PrivateCorpusEntry {",
        "  id: string;",
        "  file: string;",
        "  bytes: number;",
        '  meta: Omit<PdfMeta, "form">;',
        "  form: FormInfo | null;",
        "  formFields: Array<PageFormField>;",
        '  attachments: Array<Omit<Attachment, "data">>;',
        "  text: Array<PageText>;",
        "  nativeChars: number;",
        "  invoice: InvoiceResult;",
        "  layout: Array<PrivateCorpusPageLayout>;",
        "}",
        "",
        "export interface PrivateCorpusPageLayout {",
        "  page: number;",
        "  source: TextSource;",
        "  width: number;",
        "  height: number;",
        "  rotate: number;",
        "  words: number;",
        "  lines: number;",
        "  images: number;",
        "  fonts: Array<string>;",
        "  warnings: Array<string>;",
        "  lineText: Array<string>;",
        "}",
        "",
        "export interface PrivateCorpusReport {",
        "  generatedAt: string;",
        "  files: number;",
        "  entries: Array<PrivateCorpusEntry>;",
        "}",
        "",
        `export const privateCorpusReport = ${json} satisfies PrivateCorpusReport;`,
        ""
      ].join("\n")
    );
    t.log(`wrote ${join(PRIVATE, "report.json")} and report.ts`);
  }
);

// ── Workbooks ────────────────────────────────────────────────────────────────

const CELL_TYPES: ReadonlySet<string> = new Set(["Text", "Number", "Boolean", "Error", "Date", "Blank"]);
const CELL_REF = /^[A-Z]+[1-9]\d*$/;
const workbooks = existsSync(PRIVATE) ? discoverCorpus(PRIVATE).filter((f) => f.kind === "xlsx") : [];

(workbooks.length > 0 ? test : test.skip)(
  "private corpus: every workbook opens, its cells resolve and its logical pages hold finite ordered boxes",
  (t) => {
    t.timeout(5 * 60 * 1000, "every workbook in the private corpus through spreadsheetCells and layout");
    const started = performance.now();
    const totals = { sheets: 0, cells: 0, unsupported: 0, pages: 0, words: 0 };
    const unsupportedCodes = new Set<string>();
    for (const file of workbooks) {
      const buf = readFileSync(file.path);
      t.is(detectFormat(buf).format, "Xlsx", `${file.id}: detected as a workbook`);
      let doc: PdfDown;
      try {
        doc = new PdfDown(buf);
      } catch (error) {
        t.fail(`${file.id}: ${error instanceof Error ? error.message : String(error)}`);
        continue;
      }
      t.is(doc.format, "Xlsx", `${file.id}: format`);
      const sheets = doc.spreadsheetCells({ hiddenSheets: true });
      t.true(sheets.length >= 1, `${file.id}: at least one sheet`);
      for (const s of sheets) {
        totals.sheets += 1;
        totals.cells += s.cells.length;
        const malformed = s.cells.filter(
          (c) =>
            !CELL_TYPES.has(c.type) ||
            typeof c.text !== "string" ||
            typeof c.raw !== "string" ||
            !Number.isInteger(c.row) ||
            !Number.isInteger(c.col) ||
            !CELL_REF.test(c.ref)
        );
        t.is(malformed.length, 0, `${file.id} sheet ${s.index}: every cell is well formed`);
        for (const c of s.cells) {
          if (c.formatSupported) continue;
          totals.unsupported += 1;
          if (c.format !== undefined) unsupportedCodes.add(c.format);
        }
      }
      const pages = doc.layout();
      t.is(pages.length, sheets.filter((s) => !s.hidden).length, `${file.id}: one logical page per visible sheet`);
      for (const p of pages) {
        totals.pages += 1;
        totals.words += p.words.length;
        t.is(p.source, "Spreadsheet", `${file.id} p${p.page}: source`);
        t.truthy(p.sheet, `${file.id} p${p.page}: sheet name`);
        t.true(p.width > 0 && p.height > 0 && Number.isFinite(p.width + p.height), `${file.id} p${p.page}: page size`);
        const offPage = p.words.filter(
          (w) =>
            ![w.x0, w.y0, w.x1, w.y1].every(Number.isFinite) ||
            w.x0 > w.x1 ||
            w.y0 > w.y1 ||
            w.x0 < 0 ||
            w.y0 < 0 ||
            w.x1 > p.width + 1e-6 ||
            w.y1 > p.height + 1e-6 ||
            w.cell === undefined
        );
        t.is(offPage.length, 0, `${file.id} p${p.page}: every word is a finite box on the page carrying its cell`);
        let next = 0;
        for (const [i, line] of p.lines.entries()) {
          t.is(line.firstWord, next, `${file.id} p${p.page} line ${i}: contiguous firstWord`);
          const slice = p.words.slice(line.firstWord, line.firstWord + line.wordCount);
          t.is(slice.length, line.wordCount, `${file.id} p${p.page} line ${i}: wordCount in range`);
          t.true(
            slice.every((w) => w.line === i),
            `${file.id} p${p.page} line ${i}: words point back`
          );
          if (i > 0) {
            t.true(line.y1 <= p.lines[i - 1].y1 + 0.5, `${file.id} p${p.page} line ${i}: top-to-bottom`);
          }
          next += line.wordCount;
        }
        t.is(next, p.words.length, `${file.id} p${p.page}: lines cover every word`);
      }
    }
    const seconds = ((performance.now() - started) / 1000).toFixed(1);
    t.log(
      `${workbooks.length} workbook(s): ${totals.sheets} sheet(s), ${totals.cells} cells, ${totals.unsupported} cell(s) with number formats this release does not render (${unsupportedCodes.size} distinct codes), ${totals.pages} logical page(s), ${totals.words} words, ${seconds}s`
    );
  }
);
