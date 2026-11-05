import { readFileSync } from "fs";
import { join } from "path";

import test from "ava";

import { PdfDown, detectFormat, layoutPerPage } from "../index";

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
  // Until Task 2 lands the reader, a workbook opens no further than detection:
  await t.throwsAsync(() => PdfDown.openAsync(xlsx), { message: /workbook reading is not available yet/ });
});
