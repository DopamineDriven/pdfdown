// Local-only corpus run over real invoices in ocr/__test__/fixtures/private/.
// The directory is gitignored: the files, and anything derived from them, never
// reach the repository. The assertions here are structural; the per-file
// summary goes to ava's log, each document's full results are written beside
// its PDF as `results/<id>.json`, and report.json / report.ts (both inside the
// ignored directory) hold one index row per document: its kind (native,
// scanned, mixed, empty), its header fields and where its result file is. The
// corpus's workbooks get the same treatment with aggregates only in the log.

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "fs";
import { dirname, join, relative } from "path";

import test from "ava";

import {
  PdfDown,
  attachments,
  detectFormat,
  extractFormFieldsPerPage,
  extractImagesPerPage,
  extractInvoice,
  extractTextPerPage,
  layoutPerPage,
  pdfMetadata
} from "../index";
import type {
  Attachment,
  FormInfo,
  InvoiceDocumentType,
  InvoiceResult,
  InvoiceSource,
  PageFormField,
  PageLayout,
  PageText,
  PdfMeta,
  TextSource
} from "../index";
import { discoverCorpus } from "../scripts/lib/corpus";

/**
 * What a document is made of: `native` when every page with content carries native text,
 * `scanned` when every page with content is an image with no native text, `mixed` when both
 * kinds of page occur, `empty` when no page has words or images.
 */
type DocumentKind = "native" | "scanned" | "mixed" | "empty";

/** One raster on an image page: pixel size, the resolution it is painted at, and its codec. */
interface PrivateCorpusRaster {
  page: number;
  widthPx: number;
  heightPx: number;
  /** Pixels per inch across the painted width; null when the raster is not painted on the page. */
  ppi: number | null;
  codec: string;
}

/** One private invoice's results. Attachment payloads are omitted; everything else is verbatim. */
interface PrivateCorpusEntry {
  id: string;
  file: string;
  bytes: number;
  kind: DocumentKind;
  /** The rasters of image pages; empty for a native document. */
  rasters: Array<PrivateCorpusRaster>;
  meta: Omit<PdfMeta, "form">;
  form: FormInfo | null;
  formFields: Array<PageFormField>;
  attachments: Array<Omit<Attachment, "data">>;
  text: Array<PageText>;
  nativeChars: number;
  invoice: InvoiceResult;
  layout: Array<PrivateCorpusPageLayout>;
}

/** One index row: enough to filter the corpus without opening the document's result file. */
interface PrivateCorpusIndexRow {
  id: string;
  file: string;
  /** The document's full results, relative to the private directory. */
  results: string;
  kind: DocumentKind;
  pages: number;
  nativeChars: number;
  source: InvoiceSource;
  documentType: InvoiceDocumentType;
  confidence: number;
  invoiceId: string | null;
  invoiceDate: string | null;
  invoiceTotal: number | null;
  reconciled: boolean;
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
  kinds: Record<DocumentKind, number>;
  header: {
    invoiceId: number;
    invoiceDate: number;
    invoiceTotal: number;
    vendorName: number;
    reconciled: number;
    meanConfidence: number;
    types: Record<string, number>;
  };
  entries: Array<PrivateCorpusIndexRow>;
}

/** A page carries native text, is an image with none, or has neither. */
function pageKind(p: PageLayout): "text" | "image" | "blank" {
  if (p.words.length > 0 && p.source === "Native") return "text";
  if (p.images.length > 0) return "image";
  return "blank";
}

function documentKind(layout: Array<PageLayout>): DocumentKind {
  const kinds = layout.map(pageKind);
  const text = kinds.includes("text");
  const image = kinds.includes("image");
  if (text && image) return "mixed";
  if (text) return "native";
  if (image) return "scanned";
  return "empty";
}

/** The rasters of every image page, with the resolution each is painted at. */
function rastersOf(buf: Buffer, layout: Array<PageLayout>): Array<PrivateCorpusRaster> {
  const imagePages = new Set(layout.filter((p) => pageKind(p) === "image").map((p) => p.page));
  if (imagePages.size === 0) return [];
  const out: Array<PrivateCorpusRaster> = [];
  for (const img of extractImagesPerPage(buf)) {
    if (!imagePages.has(img.page)) continue;
    const placed = layout
      .find((p) => p.page === img.page)
      ?.images.find((b) => b.xobjectName === img.xobjectName);
    const painted = placed ? placed.x1 - placed.x0 : 0;
    out.push({
      page: img.page,
      widthPx: img.width,
      heightPx: img.height,
      ppi: painted > 0 && Number.isFinite(painted) ? Math.round((img.width / painted) * 72) : null,
      codec: img.filter
    });
  }
  return out;
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
    const rows: Array<PrivateCorpusIndexRow> = [];
    const kinds: Record<DocumentKind, number> = { native: 0, scanned: 0, mixed: 0, empty: 0 };
    const header = { invoiceId: 0, invoiceDate: 0, invoiceTotal: 0, vendorName: 0, reconciled: 0, confidenceSum: 0 };
    const types = new Map<string, number>();
    t.log(`${files.length} private invoice(s)`);
    if (notPdf.length > 0) {
      const kinds = new Map<string, number>();
      for (const s of notPdf)
        kinds.set(s.detection.detail ?? "unknown", (kinds.get(s.detection.detail ?? "unknown") ?? 0) + 1);
      t.log(`${notPdf.length} file(s) named .pdf are not PDFs: ${[...kinds].map(([k, n]) => `${k} ${n}`).join(", ")}`);
    }
    t.log(`${"file".padEnd(widths.name)}  pages  nativeChars  words  form  attach  kind     invoice`);
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
      }
      if (inv.fields.invoiceId) header.invoiceId += 1;
      if (inv.fields.invoiceDate) header.invoiceDate += 1;
      if (inv.fields.invoiceTotal) header.invoiceTotal += 1;
      if (inv.fields.vendorName) header.vendorName += 1;
      const reconciled = Boolean(inv.fields.invoiceTotal && inv.fields.subTotal && inv.confidence >= 0.7);
      if (reconciled) header.reconciled += 1;
      header.confidenceSum += inv.confidence;
      types.set(inv.documentType, (types.get(inv.documentType) ?? 0) + 1);

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
      const kind = documentKind(layout);
      kinds[kind] += 1;
      const { form, ...metaRest } = meta;
      const entry: PrivateCorpusEntry = {
        id: file.id,
        file: name,
        bytes: buf.length,
        kind,
        rasters: rastersOf(buf, layout),
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
      };
      // The document's full results live beside its PDF, one file per document.
      const resultsPath = join(dirname(file.path), "results", `${file.id}.json`);
      mkdirSync(dirname(resultsPath), { recursive: true });
      writeFileSync(resultsPath, `${JSON.stringify(entry, null, 2)}\n`);
      rows.push({
        id: file.id,
        file: name,
        results: relative(PRIVATE, resultsPath),
        kind,
        pages: meta.pageCount,
        nativeChars,
        source: inv.source,
        documentType: inv.documentType,
        confidence: inv.confidence,
        invoiceId: inv.fields.invoiceId?.value ?? null,
        invoiceDate: inv.fields.invoiceDate?.value ?? null,
        invoiceTotal: inv.fields.invoiceTotal?.value ?? null,
        reconciled
      });
      const formCol = meta.form ? `${meta.form.fieldCount}/${meta.form.widgetCount}` : "-";
      const invCol = inv.source === "EmbeddedXml" ? `${inv.source} ${inv.embeddedXml?.standard}` : inv.source;
      t.log(
        `${name.padEnd(widths.name)}  ${String(meta.pageCount).padStart(5)}  ${String(nativeChars).padStart(11)}  ${String(layoutWords).padStart(5)}  ${formCol.padStart(4)}  ${String(att.length).padStart(6)}  ${kind.padEnd(7)}  ${invCol}`
      );
    }

    const n = Math.max(rows.length, 1);
    t.log(
      `kinds: native ${kinds.native}, scanned ${kinds.scanned}, mixed ${kinds.mixed}, empty ${kinds.empty}`
    );
    t.log(
      `header track: id ${header.invoiceId}/${n}, date ${header.invoiceDate}/${n}, total ${header.invoiceTotal}/${n}, vendor ${header.vendorName}/${n}, reconciled ${header.reconciled}/${n}, mean confidence ${(header.confidenceSum / n).toFixed(2)}, types ${[...types].map(([k, v]) => `${k} ${v}`).join(" · ")}`
    );
    const report: PrivateCorpusReport = {
      generatedAt: new Date().toISOString(),
      files: rows.length,
      kinds,
      header: {
        invoiceId: header.invoiceId,
        invoiceDate: header.invoiceDate,
        invoiceTotal: header.invoiceTotal,
        vendorName: header.vendorName,
        reconciled: header.reconciled,
        meanConfidence: Number((header.confidenceSum / n).toFixed(4)),
        types: Object.fromEntries(types)
      },
      entries: rows
    };
    const json = JSON.stringify(report, null, 2);
    writeFileSync(join(PRIVATE, "report.json"), `${json}\n`);
    writeFileSync(
      join(PRIVATE, "report.ts"),
      [
        "// Generated by __test__/private-corpus.spec.ts. Lives in a gitignored directory; do not commit.",
        "// Each row's `results` names the document's full result file (results/<id>.json beside its PDF).",
        'import type { InvoiceDocumentType, InvoiceSource } from "../../../../index";',
        "",
        'export type DocumentKind = "native" | "scanned" | "mixed" | "empty";',
        "",
        "export interface PrivateCorpusIndexRow {",
        "  id: string;",
        "  file: string;",
        "  results: string;",
        "  kind: DocumentKind;",
        "  pages: number;",
        "  nativeChars: number;",
        "  source: InvoiceSource;",
        "  documentType: InvoiceDocumentType;",
        "  confidence: number;",
        "  invoiceId: string | null;",
        "  invoiceDate: string | null;",
        "  invoiceTotal: number | null;",
        "  reconciled: boolean;",
        "}",
        "",
        "export interface PrivateCorpusReport {",
        "  generatedAt: string;",
        "  files: number;",
        "  kinds: Record<DocumentKind, number>;",
        "  header: {",
        "    invoiceId: number;",
        "    invoiceDate: number;",
        "    invoiceTotal: number;",
        "    vendorName: number;",
        "    reconciled: number;",
        "    meanConfidence: number;",
        "    types: Record<string, number>;",
        "  };",
        "  entries: Array<PrivateCorpusIndexRow>;",
        "}",
        "",
        `export const privateCorpusReport = ${json} satisfies PrivateCorpusReport;`,
        ""
      ].join("\n")
    );
    t.log(`wrote ${join(PRIVATE, "report.json")} and report.ts; one results/<id>.json per document`);
  }
);

// ── Workbooks ────────────────────────────────────────────────────────────────

const CELL_TYPES: ReadonlySet<string> = new Set(["Text", "Number", "Boolean", "Error", "Date", "Time", "Blank"]);
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
