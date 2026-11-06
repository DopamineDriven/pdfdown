import { readFileSync } from "fs";
import { join } from "path";

import test from "ava";

import {
  PdfDown,
  detectFormat,
  detectFormatAsync,
  layoutFromXlsx,
  layoutPerPage,
  spreadsheetCells,
  spreadsheetCellsAsync
} from "../index";
import type { SheetCell } from "../index";

const FX = join(import.meta.dirname, "fixtures");
const pdf = readFileSync(join(FX, "layout", "layout-sample.pdf"));
const xlsx = readFileSync(join(FX, "spreadsheet", "minimal.xlsx"));
const read = (name: string) => readFileSync(join(FX, "spreadsheet", name));

test("detectFormat names the format and the reason an input is unsupported", (t) => {
  t.deepEqual(detectFormat(pdf), { format: "Pdf" });
  t.deepEqual(detectFormat(xlsx), { format: "Xlsx" });
  t.deepEqual(detectFormat(read("not-a-workbook.docx")), { format: "Unknown", detail: "Word document package" });
  t.is(detectFormat(read("ole.bin")).format, "Unknown");
  t.is(detectFormat(Buffer.from("hello")).detail, "not a PDF or a zip package");
});

test("openAsync resolves to a PdfDown whose methods work; a PDF and a workbook opened concurrently keep their own state", async (t) => {
  const [a, b] = await Promise.all([PdfDown.openAsync(pdf), PdfDown.openAsync(form)]);
  t.true(a instanceof PdfDown && b instanceof PdfDown);
  t.is(a.format, "Pdf");
  t.is(b.format, "Xlsx");
  t.is(a.metadata().pageCount, 2);
  t.deepEqual(await a.layoutAsync(), layoutPerPage(pdf));
  t.is((await a.textPerPageAsync()).length, 2);
  t.is(b.spreadsheetCells()[0].sheet, "Form");
  t.deepEqual(await b.layoutAsync(), layoutFromXlsx(form));
  t.throws(() => b.metadata(), { message: /unsupported operation: metadata reads a PDF; this document is a workbook/ });
  t.throws(() => a.spreadsheetCells(), { message: /unsupported operation: spreadsheetCells reads a workbook/ });
});

test("a PDF-only method on a workbook is an unsupported operation, and opening an unsupported package fails with its detail", async (t) => {
  await t.throwsAsync(() => PdfDown.openAsync(read("macro.xlsm")), {
    message: /unsupported format: macro-enabled workbook/
  });
  t.throws(() => new PdfDown(read("ole.bin")), { message: /unsupported format: OLE container/ });
  t.is((await PdfDown.openAsync(xlsx)).format, "Xlsx");
});

test("detectFormatAsync agrees with detectFormat and runs off the main thread", async (t) => {
  t.deepEqual(await detectFormatAsync(pdf), detectFormat(pdf));
  t.deepEqual(await detectFormatAsync(xlsx), { format: "Xlsx" });
  t.deepEqual(await detectFormatAsync(read("not-a-workbook.docx")), {
    format: "Unknown",
    detail: "Word document package"
  });
});

test("async methods on the wrong document kind reject; nothing throws before the promise exists", async (t) => {
  const wb = new PdfDown(form);
  for (const [name, call] of [
    ["textPerPageAsync", () => wb.textPerPageAsync()],
    ["imagesPerPageAsync", () => wb.imagesPerPageAsync()],
    ["annotationsPerPageAsync", () => wb.annotationsPerPageAsync()],
    ["attachmentsAsync", () => wb.attachmentsAsync()],
    ["metadataAsync", () => wb.metadataAsync()],
    ["documentAsync", () => wb.documentAsync()],
    ["structuredTextAsync", () => wb.structuredTextAsync()],
    ["formFieldsPerPageAsync", () => wb.formFieldsPerPageAsync()]
  ] as const) {
    await t.throwsAsync(call, {
      message: new RegExp(`^unsupported operation: ${name} reads a PDF; this document is a workbook$`)
    });
  }
  await t.throwsAsync(() => new PdfDown(pdf).spreadsheetCellsAsync(), {
    message: /^unsupported operation: spreadsheetCellsAsync reads a workbook; this document is a PDF$/
  });
});

test("a zip past the input cap is a limit error, not an unsupported format", (t) => {
  const big = Buffer.alloc(65 * 1024 * 1024);
  big.write("PK\u0003\u0004", 0, "latin1");
  t.throws(() => new PdfDown(big), { message: /^limit exceeded: the package is \d+ bytes/ });
  t.throws(() => spreadsheetCells(big), { message: /^limit exceeded: / });
  t.like(detectFormat(big), { format: "Unknown" });
  t.regex(detectFormat(big).detail ?? "", /limit/);
});

// ── Cells ────────────────────────────────────────────────────────────────────

const form = readFileSync(join(FX, "spreadsheet", "form.xlsx"));
const cell = (cells: ReadonlyArray<SheetCell>, ref: string) => cells.find((c) => c.ref === ref);

test("spreadsheetCells: strings, numbers, booleans, errors, formulas and merges, visible sheets by default", (t) => {
  const doc = new PdfDown(form);
  t.is(doc.format, "Xlsx");
  const sheets = doc.spreadsheetCells();
  t.deepEqual(
    sheets.map((s) => [s.sheet, s.index, s.hidden]),
    [["Form", 0, false]]
  );
  const [s] = sheets;
  t.is(s.dimension, "A1:H30");
  t.is(s.printArea, "A1:F12");
  t.deepEqual(s.merges, ["A1:F1", "A11:A12"]);
  const c = s.cells;
  t.like(cell(c, "A1"), {
    type: "Text",
    value: "Maintenance Reimbursement Form",
    text: "Maintenance Reimbursement Form",
    merge: "A1:F1",
    row: 0,
    col: 0
  });
  t.like(cell(c, "C3"), {
    type: "Number",
    value: 142.5,
    raw: "142.5",
    format: '#,##0.00"/hr"',
    formula: undefined,
    cached: true
  });
  t.like(cell(c, "C5"), { type: "Number", value: 427.5, formula: "C3*C4", cached: true });
  t.like(cell(c, "C7"), { type: "Text", value: "000123", text: "000123", raw: "000123" });
  t.like(cell(c, "B16"), { formula: "C3*2", sharedFormula: undefined, type: "Number", value: 285, cached: true });
  t.like(cell(c, "B17"), { type: "Text", value: "", text: "", sharedFormula: "B16", formula: undefined, cached: true });
  t.like(cell(c, "B18"), { type: "Blank", text: "", sharedFormula: "B16", cached: false });
  t.like(cell(c, "D20"), { type: "Error", value: "#DIV/0!", text: "#DIV/0!" });
  t.like(cell(c, "E21"), { type: "Boolean", value: true, text: "TRUE" });
  t.like(cell(c, "B10"), { text: "hidden value", hidden: true });
  t.like(cell(c, "A11"), { text: "Line one\nLine two", merge: "A11:A12" });
  t.is(cell(c, "G20"), undefined, "a styled empty cell is not a cell");
  t.true(
    s.warnings.some((w) => /C22.*serial 60/.test(w)),
    s.warnings.join(" | ")
  );
});

test("spreadsheetCells options: hidden sheets and hidden cells", (t) => {
  const doc = new PdfDown(form);
  t.deepEqual(
    doc.spreadsheetCells({ hiddenSheets: true }).map((s) => s.sheet),
    ["Form", "Notes"]
  );
  t.deepEqual(
    doc.spreadsheetCells({ sheets: ["Notes"], hiddenSheets: true })[0].cells.map((c) => c.text),
    ["secret"]
  );
  t.is(cell(doc.spreadsheetCells({ hiddenCells: false })[0].cells, "B10"), undefined);
  t.is(
    new PdfDown(xlsx).spreadsheetCells()[0].cells.length,
    4,
    "no shared strings and no styles still resolves every cell"
  );
  t.throws(() => new PdfDown(pdf).spreadsheetCells(), {
    message: /unsupported operation: spreadsheetCells reads a workbook/
  });
});

test("Number cells carry a subtype and the currency symbol their format names", (t) => {
  const [s] = new PdfDown(form).spreadsheetCells();
  t.like(cell(s.cells, "C3"), { type: "Number", subtype: "Float", currencySymbol: undefined });
  t.like(cell(s.cells, "C4"), { type: "Number", subtype: "Integer" });
  t.like(cell(s.cells, "C8"), { type: "Number", subtype: "Integer", currencySymbol: "$" });
  t.like(cell(s.cells, "C9"), { type: "Number", subtype: "Float" });
  t.like(cell(s.cells, "B16"), { type: "Number", subtype: "Integer" });
  t.is(cell(s.cells, "C6")?.type, "Date");
});

test("spreadsheetCells carries the sheet's geometry: defaults, the columns and rows that differ, and a box on every cell", (t) => {
  const [s] = new PdfDown(form).spreadsheetCells();
  // The sheet frame runs from A1 to the last non-empty cell or merged range (A1:F22 here), y up.
  t.like(s, { width: 385.5, height: 345, defaultColumnWidth: 48, defaultRowHeight: 15 });
  t.deepEqual(s.columns, [
    { index: 0, width: 105, hidden: false },
    { index: 1, width: 63, hidden: false },
    { index: 2, width: 73.5, hidden: false }
  ]);
  t.deepEqual(s.rows, [
    { index: 9, height: 15, hidden: true },
    { index: 10, height: 30, hidden: false }
  ]);
  t.deepEqual(cell(s.cells, "C3")?.box, { x0: 168, y0: 300, x1: 241.5, y1: 315 });
  t.deepEqual(cell(s.cells, "A1")?.box, { x0: 0, y0: 330, x1: 385.5, y1: 345 }, "a merged anchor spans its range");
  t.deepEqual(cell(s.cells, "A11")?.box, { x0: 0, y0: 150, x1: 105, y1: 195 });
  t.true(
    s.cells.every((c) => c.box.x0 <= c.box.x1 && c.box.y0 < c.box.y1 && c.box.x1 <= s.width && c.box.y1 <= s.height)
  );
  const [visible] = new PdfDown(form).spreadsheetCells({ hiddenCells: false });
  t.is(visible.height, 330, "a hidden row has no extent when hidden cells are excluded");
  t.deepEqual(visible.rows, [
    { index: 9, height: 0, hidden: true },
    { index: 10, height: 30, hidden: false }
  ]);
  t.deepEqual(new PdfDown(xlsx).spreadsheetCells()[0].columns, [], "no column differs from the default");
});

test("spreadsheetCellsAsync and the standalone function agree", async (t) => {
  const doc = await PdfDown.openAsync(form);
  t.deepEqual(await doc.spreadsheetCellsAsync(), spreadsheetCells(form));
  t.deepEqual(await spreadsheetCellsAsync(form, { hiddenSheets: true }), doc.spreadsheetCells({ hiddenSheets: true }));
});

test("spreadsheetCells: display text follows the format code; dates are typed", (t) => {
  const [s] = new PdfDown(form).spreadsheetCells();
  const texts = ["C3", "C4", "C5", "C6", "C8", "C9"].map((r) => cell(s.cells, r)?.text);
  t.deepEqual(texts, ["142.50/hr", "3.0", "427.50", "8/31/2015", "($5.00)", "7.50%"]);
  t.like(cell(s.cells, "C6"), { type: "Date", value: "2015-08-31", raw: "42247", formatSupported: true });
  t.like(cell(s.cells, "C22"), { type: "Number", value: 60, text: "60", formatSupported: true });
  t.like(new PdfDown(readFileSync(join(FX, "spreadsheet", "date1904.xlsx"))).spreadsheetCells()[0].cells[0], {
    type: "Date",
    value: "1904-01-01",
    raw: "0",
    text: "1/1/1904"
  });
  t.like(cell(new PdfDown(xlsx).spreadsheetCells()[0].cells, "B1"), { text: "427.5", formatSupported: true });
});

// ── Layout ───────────────────────────────────────────────────────────────────

test("layout on a workbook: one logical page per visible sheet, words carry their cell, geometry from widths and heights", (t) => {
  const doc = new PdfDown(form);
  const pages = doc.layout();
  t.is(pages.length, 1);
  const [p] = pages;
  t.like(p, { page: 1, source: "Spreadsheet", sheet: "Form", rotate: 0, contentRotation: 0, images: [] });
  t.is(p.width, 385.5, "A 20 + B 12 + C 14 + three default columns of 48 pt");
  t.is(p.height, 180, "rows 1..12 at 15 pt, row 11 at 30 pt, hidden row 10 omitted");
  const word = (text: string) => p.words.find((w) => w.text === text);
  t.like(word("142.50/hr"), { cell: "C3", fontName: "Arial", fontSize: 10, bold: true, confidence: undefined });
  const rate = word("142.50/hr");
  t.true(
    rate !== undefined && Math.abs(rate.x1 - 241.5) < 0.01 && Math.abs(rate.x0 - 196.5) < 0.01,
    "numbers in General alignment sit on the column's right edge"
  );
  t.deepEqual(
    p.words.filter((w) => w.cell === "A11").map((w) => w.text),
    ["Line", "one", "Line", "two"]
  );
  t.true(p.words.every((w) => w.x0 >= 0 && w.x1 <= p.width && w.y0 >= 0 && w.y1 <= p.height && w.cell !== undefined));
  t.is(word("hidden value"), undefined);
  t.true(p.warnings.some((w) => /print area A1:F12/.test(w)));
  for (let i = 1; i < p.lines.length; i++) t.true(p.lines[i].y1 <= p.lines[i - 1].y1 + 0.5);
});

test("layout options on a workbook: hidden cells restore their extents; hidden sheets add pages; the standalone agrees", async (t) => {
  const doc = new PdfDown(form);
  const grown = doc.layout({ hiddenCells: true })[0];
  t.is(grown.height, 195);
  t.is(grown.words.find((w) => w.text === "hidden")?.cell, "B10");
  t.deepEqual(
    doc.layout({ hiddenSheets: true }).map((p) => [p.page, p.sheet]),
    [
      [1, "Form"],
      [2, "Notes"]
    ]
  );
  t.deepEqual(await doc.layoutAsync({ sheets: ["Form"] }), layoutFromXlsx(form, { sheets: ["Form"] }));
  t.deepEqual(layoutPerPage(pdf), new PdfDown(pdf).layout(), "PDF layout ignores the options");
});

test("invoice on a workbook returns the no-source shape until the shared engine exists", async (t) => {
  const result = await new PdfDown(form).invoiceAsync();
  t.like(result, { source: "None", documentType: "Unknown", confidence: 0 });
  t.deepEqual(result.fields.items, []);
  t.true(result.warnings.some((w) => /layout-based extraction is not available yet/.test(w)));
});
