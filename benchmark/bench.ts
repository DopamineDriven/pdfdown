import { Bench } from "tinybench";
import { readFileSync } from "fs";
import { join } from "path";

import {
  extractImagesPerPage,
  extractFormFieldsPerPage,
  extractImagesPerPageAsync,
  extractTextPerPage,
  extractTextPerPageAsync,
  pdfMetadata,
  pdfMetadataAsync
} from "../index.js";

const PDF_URL =
  "https://assets-dev.aicoalesce.com/upload/nrr6h4r4480f6kviycyo1zhf/1765025311330-Candy-Flipping-Claudtullus-Pt-I.pdf";

const res = await fetch(PDF_URL);
if (!res.ok) throw new Error(`Failed to fetch PDF: ${res.status} ${res.statusText}`);
const pdf = Buffer.from(await res.arrayBuffer());

const b = new Bench();

const FORMS = join(import.meta.dirname, "..", "__test__", "fixtures", "forms");
const acroformBasic = readFileSync(join(FORMS, "acroform-basic.pdf"));
const acroformMany = readFileSync(join(FORMS, "acroform-many.pdf"));

b.add("extractFormFieldsPerPage (basic, 11 widgets)", () => {
  return extractFormFieldsPerPage(acroformBasic);
});

b.add("extractFormFieldsPerPage (many, 1500 widgets / 30 pages)", () => {
  return extractFormFieldsPerPage(acroformMany);
});

b.add("extractTextPerPage (sync)", () => {
  return extractTextPerPage(pdf);
});

b.add("extractTextPerPageAsync", async () => {
  return await extractTextPerPageAsync(pdf);
});

b.add("extractImagesPerPage (sync)", () => {
  return extractImagesPerPage(pdf);
});

b.add("extractImagesPerPageAsync", async () => {
  return await extractImagesPerPageAsync(pdf);
});

b.add("pdfMetadata (sync)", () => {
  return pdfMetadata(pdf);
});

b.add("pdfMetadataAsync", async () => {
  return await pdfMetadataAsync(pdf);
});

b.run().then((_tasks) => {
  console.table(b.table());
});
