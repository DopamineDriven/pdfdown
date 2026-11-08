// Run the invoice reader over an Azure Document Intelligence read result:
// `yarn invoice:from-azure <analyze-result.json> [--layout]`.
//
// The JSON is the analyze result (or an object holding it under `analyzeResult`); its pages are
// mapped onto pdfdown pages and `invoiceFromLayout` reads them. Prints the header fields with
// their confidence and span page, and the warnings; `--layout` also prints each page's lines.
import { readFileSync } from "fs";

import { invoiceFromLayout } from "../index";
import { azureToLayout } from "./lib/azure-layout";
import type { AzureReadResult } from "./lib/azure-layout";

const args = process.argv.slice(2);
const file = args.find((a) => !a.startsWith("--"));
if (file === undefined) {
  console.error("usage: yarn invoice:from-azure <analyze-result.json> [--layout]");
  process.exit(2);
}
const raw: unknown = JSON.parse(readFileSync(file, "utf8"));
const result = (
  typeof raw === "object" && raw !== null && "analyzeResult" in raw
    ? (raw as { analyzeResult: AzureReadResult }).analyzeResult
    : (raw as AzureReadResult)
) satisfies AzureReadResult;

const pages = azureToLayout(result);
if (args.includes("--layout")) {
  for (const p of pages) {
    console.log(`== page ${p.page} ${p.width.toFixed(0)}x${p.height.toFixed(0)}pt, ${p.words.length} words`);
    for (const l of p.lines) console.log(`   ${l.text}`);
  }
}
const r = invoiceFromLayout(pages);
console.log(`source ${r.source}, type ${r.documentType}, confidence ${r.confidence.toFixed(2)}`);
const f = r.fields;
const show = (name: string, field: { value: unknown; confidence: number; span?: { page: number } } | undefined) =>
  console.log(
    `${name.padEnd(16)} ${field ? `${JSON.stringify(field.value)} @${field.confidence.toFixed(2)} p${field.span?.page ?? "?"}` : "-"}`
  );
show("invoiceId", f.invoiceId);
show("invoiceDate", f.invoiceDate);
show("dueDate", f.dueDate);
show("purchaseOrder", f.purchaseOrder);
show("workOrder", f.workOrder);
show("vendorName", f.vendorName);
show("customerName", f.customerName);
show("subTotal", f.subTotal);
show("totalTax", f.totalTax);
show("invoiceTotal", f.invoiceTotal);
show("amountDue", f.amountDue);
console.log(`currency ${f.currencyCode ?? "-"} (${r.currencySource ?? "-"})`);
for (const a of f.aircraft) {
  show("registration", a.registration);
  show("serialNumber", a.serialNumber);
  show("engineSerial", a.engineSerialNumber);
}
for (const w of r.warnings) console.log(`warning: ${w}`);
