import { readFileSync } from "fs";
import { join } from "path";

import test from "ava";

import { PdfDown, detectFormat, layoutPerPage, spreadsheetCells, spreadsheetCellsAsync } from "../index";
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

test("openAsync resolves to a PdfDown whose PDF methods work, and two instances do not interfere", async (t) => {
  const [a, b] = await Promise.all([PdfDown.openAsync(pdf), PdfDown.openAsync(pdf)]);
  t.true(a instanceof PdfDown);
  t.is(a.format, "Pdf");
  t.is(a.metadata().pageCount, b.metadata().pageCount);
  t.deepEqual(await a.layoutAsync(), layoutPerPage(pdf));
  t.is((await b.textPerPageAsync()).length, 2);
});

test("a PDF-only method on a workbook is an unsupported operation, and opening an unsupported package fails with its detail", async (t) => {
  await t.throwsAsync(() => PdfDown.openAsync(read("macro.xlsm")), {
    message: /unsupported format: macro-enabled workbook/
  });
  t.throws(() => new PdfDown(read("ole.bin")), { message: /unsupported format: OLE container/ });
  t.is((await PdfDown.openAsync(xlsx)).format, "Xlsx");
});

// ── Cells ────────────────────────────────────────────────────────────────────

const form = readFileSync(join(FX, "spreadsheet", "form.xlsx"));
const cell = (cells: ReadonlyArray<SheetCell>, ref: string) => cells.find((c) => c.ref === ref);

test("spreadsheetCells: strings, numbers, booleans, errors, formulas and merges, visible sheets by default", (t) => {
  const doc = new PdfDown(form);
  t.is(doc.format, "Xlsx");
  const sheets = doc.spreadsheetCells();
  t.deepEqual(sheets.map((s) => [s.sheet, s.index, s.hidden]), [["Form", 0, false]]);
  const [s] = sheets;
  t.is(s.dimension, "A1:H30");
  t.is(s.printArea, "A1:F12");
  t.deepEqual(s.merges, ["A1:F1", "A11:A12"]);
  const c = s.cells;
  t.like(cell(c, "A1"), { type: "Text", value: "Maintenance Reimbursement Form", text: "Maintenance Reimbursement Form", merge: "A1:F1", row: 0, col: 0 });
  t.like(cell(c, "C3"), { type: "Number", value: 142.5, raw: "142.5", format: '#,##0.00"/hr"', formula: undefined, cached: true });
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
  t.true(s.warnings.some((w) => /C22.*serial 60/.test(w)), s.warnings.join(" | "));
});

test("spreadsheetCells options: hidden sheets and hidden cells", (t) => {
  const doc = new PdfDown(form);
  t.deepEqual(doc.spreadsheetCells({ hiddenSheets: true }).map((s) => s.sheet), ["Form", "Notes"]);
  t.deepEqual(doc.spreadsheetCells({ sheets: ["Notes"], hiddenSheets: true })[0].cells.map((c) => c.text), ["secret"]);
  t.is(cell(doc.spreadsheetCells({ hiddenCells: false })[0].cells, "B10"), undefined);
  t.is(new PdfDown(xlsx).spreadsheetCells()[0].cells.length, 4, "no shared strings and no styles still resolves every cell");
  t.throws(() => new PdfDown(pdf).spreadsheetCells(), { message: /unsupported operation: spreadsheetCells reads a workbook/ });
});

test("spreadsheetCellsAsync and the standalone function agree", async (t) => {
  const doc = await PdfDown.openAsync(form);
  t.deepEqual(await doc.spreadsheetCellsAsync(), spreadsheetCells(form));
  t.deepEqual(await spreadsheetCellsAsync(form, { hiddenSheets: true }), doc.spreadsheetCells({ hiddenSheets: true }));
});
