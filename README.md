# `@d0paminedriven/pdfdown`

![CI](https://github.com/DopamineDriven/pdfdown/workflows/CI/badge.svg)

Rust-powered PDF extraction for Node.js via [napi-rs](https://napi.rs). Extracts per-page text, images (as PNG), annotations (links, destinations), structured text (with header/footer detection), AcroForm fields (read-only, one entry per widget placement), and metadata from PDF buffers. All extraction is parallelized with [rayon](https://github.com/rayon-rs/rayon) for multi-core performance.

An OCR variant is available as [`@d0paminedriven/pdfdown-ocr`](https://www.npmjs.com/package/@d0paminedriven/pdfdown-ocr) for image-only PDF pages (scanned documents, embedded screenshots). It includes all of the APIs below plus Tesseract-based OCR fallback.

## Install

```bash
npm install @d0paminedriven/pdfdown
# or
yarn add @d0paminedriven/pdfdown
# or
pnpm add @d0paminedriven/pdfdown
```

## API

### Standalone functions

#### Synchronous

```typescript
export declare function extractTextPerPage(buffer: Buffer): Array<PageText>;
export declare function extractImagesPerPage(buffer: Buffer): Array<PageImage>;
export declare function extractAnnotationsPerPage(buffer: Buffer): Array<PageAnnotation>;
export declare function attachments(buffer: Buffer): Array<Attachment>;
export declare function extractInvoice(buffer: Buffer, opts?: InvoiceOptions): InvoiceResult;
export declare function layoutPerPage(buffer: Buffer): Array<PageLayout>;
export declare function extractStructuredTextPerPage(buffer: Buffer): Array<StructuredPageText>;
export declare function pdfMetadata(buffer: Buffer): PdfMeta;
export declare function pdfDocument(buffer: Buffer): PdfDocument;
export declare function extractFormFieldsPerPage(buffer: Buffer): Array<PageFormField>;

// Any buffer: what it is. Workbooks (.xlsx): typed cells and logical pages.
export declare function detectFormat(buffer: Buffer): FormatDetection;
export declare function spreadsheetCells(buffer: Buffer, opts?: SpreadsheetOptions): Array<SheetCells>;
export declare function layoutFromXlsx(buffer: Buffer, opts?: SpreadsheetOptions): Array<PageLayout>;
```

#### Async (libuv thread pool)

Each sync function has an async counterpart that runs on the libuv thread pool, keeping the main thread free.

```typescript
export declare function extractTextPerPageAsync(buffer: Buffer): Promise<Array<PageText>>;
export declare function extractImagesPerPageAsync(buffer: Buffer): Promise<Array<PageImage>>;
export declare function extractAnnotationsPerPageAsync(buffer: Buffer): Promise<Array<PageAnnotation>>;
export declare function attachmentsAsync(buffer: Buffer): Promise<Array<Attachment>>;
export declare function extractInvoiceAsync(buffer: Buffer, opts?: InvoiceOptions): Promise<InvoiceResult>;
export declare function layoutPerPageAsync(buffer: Buffer): Promise<Array<PageLayout>>;
export declare function extractStructuredTextPerPageAsync(buffer: Buffer): Promise<Array<StructuredPageText>>;
export declare function pdfMetadataAsync(buffer: Buffer): Promise<PdfMeta>;
export declare function pdfDocumentAsync(buffer: Buffer): Promise<PdfDocument>;
export declare function extractFormFieldsPerPageAsync(buffer: Buffer): Promise<Array<PageFormField>>;
export declare function detectFormatAsync(buffer: Buffer): Promise<FormatDetection>;
export declare function spreadsheetCellsAsync(buffer: Buffer, opts?: SpreadsheetOptions): Promise<Array<SheetCells>>;
export declare function layoutFromXlsxAsync(buffer: Buffer, opts?: SpreadsheetOptions): Promise<Array<PageLayout>>;
```

### `PdfDown` class

Parse once, call many. The constructor parses the document once, a PDF or an `.xlsx` workbook, and reuses it across all sync method calls; `openAsync` does the same parse off the JavaScript thread. Async methods share the parsed document across worker threads via `Arc` (atomic reference counting) with zero re-parsing overhead. `format` says which kind was opened; a sync method that reads the other kind throws an `unsupported operation` error and an async one rejects with it, so nothing throws before the promise exists (see [Workbooks](#workbooks)).

```typescript
export declare class PdfDown {
  constructor(buffer: Buffer);
  static openAsync(buffer: Buffer): Promise<PdfDown>;
  readonly format: DocumentFormat; // "Pdf" | "Xlsx"
  textPerPage(): Array<PageText>;
  imagesPerPage(): Array<PageImage>;
  annotationsPerPage(): Array<PageAnnotation>;
  attachments(): Array<Attachment>;
  invoice(opts?: InvoiceOptions): InvoiceResult;
  layout(opts?: SpreadsheetOptions): Array<PageLayout>; // a PDF ignores opts; a workbook yields one logical page per sheet
  spreadsheetCells(opts?: SpreadsheetOptions): Array<SheetCells>; // workbooks only
  structuredText(): Array<StructuredPageText>;
  metadata(): PdfMeta;
  document(): PdfDocument;
  textPerPageAsync(): Promise<Array<PageText>>;
  imagesPerPageAsync(): Promise<Array<PageImage>>;
  annotationsPerPageAsync(): Promise<Array<PageAnnotation>>;
  attachmentsAsync(): Promise<Array<Attachment>>;
  invoiceAsync(opts?: InvoiceOptions): Promise<InvoiceResult>;
  layoutAsync(opts?: SpreadsheetOptions): Promise<Array<PageLayout>>;
  spreadsheetCellsAsync(opts?: SpreadsheetOptions): Promise<Array<SheetCells>>;
  structuredTextAsync(): Promise<Array<StructuredPageText>>;
  metadataAsync(): Promise<PdfMeta>;
  documentAsync(): Promise<PdfDocument>;
  formFieldsPerPage(): Array<PageFormField>;
  formFieldsPerPageAsync(): Promise<Array<PageFormField>>;
}
```

### Types

```typescript
export interface PageText {
  page: number;
  text: string;
}

export interface StructuredPageText {
  page: number;
  header: string;
  body: string;
  footer: string;
}

// Positioned text. All coordinates are PDF user space in points, y up,
// exactly as the content stream placed them (not shifted to the CropBox
// origin, not rotated by /Rotate).
export interface PageLayout {
  page: number;
  width: number; // CropBox (fallback MediaBox) size in points
  height: number;
  rotate: number; // /Rotate, reported and not applied
  contentRotation: number; // 0 | 90 | 180 | 270: the page frame's turn, the one carrying the most confident OCR text across its images; words, lines and images all share it; the render tier's turn includes /Rotate (always 0 for Native). Displayed coordinates = this frame turned by rotate - contentRotation
  source: TextSource; // "Native" for a PDF here; "Ocr" | "Rendered" from the OCR package: the deepest cascade tier that ran, native words kept alongside; "Spreadsheet" for a workbook's logical page
  sheet?: string; // spreadsheet pages only: the sheet this page projects
  words: Array<TextWord>;
  lines: Array<TextLine>;
  images: Array<PlacedImage>;
  warnings: Array<string>;
}

export interface TextWord {
  text: string;
  x0: number;
  y0: number;
  x1: number;
  y1: number;
  line: number; // index into PageLayout.lines
  fontSize?: number; // native only: size in points after the CTM
  fontName?: string; // native only: base font, subset prefix stripped
  bold?: boolean; // native only: name, ForceBold flag or FontWeight >= 600; absent when the font gives no weight evidence (anonymous CIDFont+Fn names)
  italic?: boolean;
  symbolic?: boolean; // native only: set in a barcode font, so the glyphs are bars, not letters
  confidence?: number; // OCR only: 0..1
  cell?: string; // spreadsheet only: the cell this word was laid out from ("C3"); the anchor of a merged range
}

export interface TextLine {
  text: string; // words joined by single spaces
  x0: number;
  y0: number;
  x1: number;
  y1: number;
  firstWord: number; // PageLayout.words[firstWord .. firstWord + wordCount) are this line's words
  wordCount: number;
}

export interface PlacedImage {
  xobjectName: string; // matches PageImage.xobjectName
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

export type TextSource = "Native" | "Ocr" | "Rendered" | "Spreadsheet";

export interface PageImage {
  page: number;
  imageIndex: number;
  width: number;
  height: number;
  data: Buffer; // PNG-encoded bytes
  colorSpace: string;
  bitsPerComponent: number;
  filter: string;
  xobjectName: string;
  objectId: string;
}

export interface PageAnnotation {
  page: number;
  subtype: string; // "Link", "Text", "Highlight", etc.
  rect: Array<number>; // [x1, y1, x2, y2] bounding box
  uri?: string; // external link URL
  dest?: string; // internal named destination
  content?: string; // tooltip / alt text
}

export const enum BoxType {
  CropBox = "CropBox",
  MediaBox = "MediaBox",
  Unknown = "Unknown"
}

export interface PageBox {
  pageCount: number; // number of pages with these dimensions
  left: number;
  bottom: number;
  right: number;
  top: number;
  width: number;
  height: number;
  boxType: BoxType;
  pages?: Array<number>; // only present on non-dominant entries
}

export interface PdfMeta {
  pageCount: number;
  version: string;
  isLinearized: boolean;
  creator?: string;
  producer?: string;
  creationDate?: string;
  modificationDate?: string;
  pageBoxes: Array<PageBox>;
  form?: FormInfo; // present when /AcroForm exists or orphan widgets were found
}

export interface PdfDocument {
  version: string;
  isLinearized: boolean;
  pageCount: number;
  creator?: string;
  producer?: string;
  creationDate?: string;
  modificationDate?: string;
  pageBoxes: Array<PageBox>;
  totalImages: number;
  totalAnnotations: number;
  imagePages: Array<number>;
  annotationPages: Array<number>;
  text: Array<PageText>;
  structuredText: Array<StructuredPageText>;
  images: Array<PageImage>;
  annotations: Array<PageAnnotation>;
  form?: FormInfo;
  formFields: Array<PageFormField>; // one entry per widget placement, sorted by page
  totalFormFields: number; // === formFields.length
  formFieldPages: Array<number>; // sorted unique pages carrying widgets
}
```

### Form field types

`PageFormField` is a discriminated union keyed by `type`: one entry per **widget placement**, so a field shown on two pages yields two entries and a radio group yields one entry per option. Join placements of one logical field on `fieldObjectId`. Presentation data (`/DA`, `/Q`, `/MK`, appearance streams) is deliberately not surfaced: pdfdown reports field data and geometry, never how a viewer should draw it, and it never writes to a form.

```typescript
export type PageFormField =
  | {
      type: "Text";
      /* common */ multiline: boolean;
      password: boolean;
      comb: boolean;
      richText: boolean;
      maxLen?: number;
    }
  | { type: "Checkbox"; /* common */ onState?: string; exportValue?: string; checked: boolean }
  | { type: "Radio"; /* common */ onState?: string; exportValue?: string; selected: boolean }
  | { type: "ComboBox"; /* common */ options: Array<ChoiceOption>; editable: boolean }
  | { type: "ListBox"; /* common */ options: Array<ChoiceOption>; multiSelect: boolean; values?: Array<string> }
  | { type: "PushButton" /* common */ }
  | { type: "Signature"; /* common */ signed: boolean } // signature data present in /V, not cryptographic validation
  | { type: "Unknown" /* common */ }; // widget annotations no field claims (orphans, or no /AcroForm at all)

// "common" on every variant:
//   page: number;          // 1-based, like PageText / PageImage / PageAnnotation
//   rect: Array<number>;   // [x0, y0, x1, y1] PDF user space, same convention as PageAnnotation.rect
//   objectId: string;      // this widget, "N G obj" like PageImage.objectId ("page P annot I" for an inline dictionary)
//   fieldObjectId: string; // the logical field; identical on every placement of one field
//   name: string;          // fully qualified "parent.kid"
//   partialName: string; widgetIndex: number; widgetCount: number;
//   value?: string; defaultValue?: string; tooltip?: string; mappingName?: string;
//   readOnly: boolean; required: boolean; noExport: boolean; hasActions: boolean

export interface ChoiceOption {
  exportValue: string;
  displayValue: string;
}

export interface FormInfo {
  hasAcroForm: boolean; // /AcroForm present (false when only orphan widgets were found)
  hasXfa: boolean; // /XFA present; XFA is not parsed
  needAppearances: boolean;
  sigFlags: number;
  fieldCount: number; // logical fields, placed + unplaced; may differ from totalFormFields
  widgetCount: number; // placements; always equals totalFormFields
  unplacedFields: Array<UnplacedFormField>; // fields with no widget on any page: data kept, no geometry invented
  truncated: boolean; // the /Kids depth cap (100) was hit: the result is partial
  warnings: Array<string>; // dangling references, non-array /Annots, non-dictionary /Kids entries, depth cap
}

export interface UnplacedFormField {
  fieldObjectId: string;
  name: string;
  partialName: string;
  type: FormFieldType;
  value?: string;
  values?: Array<string>;
  defaultValue?: string;
  tooltip?: string;
  mappingName?: string;
  options?: Array<ChoiceOption>;
  maxLen?: number;
  readOnly: boolean;
  required: boolean;
  noExport: boolean;
  hasActions: boolean;
  multiline: boolean;
  password: boolean;
  comb: boolean;
  richText: boolean;
  editable: boolean;
  multiSelect: boolean;
  signed: boolean;
}
```

Buttons keep the spec's two representations apart: `onState` is the appearance state name from the widget's `/AP /N` (absent, never invented, when there is no appearance dictionary) and `exportValue` is the `/Opt` entry that state maps to. `checked` / `selected` follow `/AS`, then `/V`; `value` always reports `/V`, so a stored value and an appearance state that disagree are both visible.

### Utility types

Every `index.d.ts` starts with a small set of type utilities (`Rm`, `XOR`, `CTR`, `RTC`, `UTR`, `IsExact`, `Equal`, …) and three aliases derived from the generated declarations:

```typescript
export type FormFieldType = PageFormField["type"]; // "Text" | "Checkbox" | …
export type FormFieldByType = UTR<PageFormField, "type", true>; // { Text: {…}, Checkbox: {…}, … } without the discriminant
export type FormFieldPayload<T extends keyof FormFieldByType = keyof FormFieldByType> = {
  [P in T]: FormFieldByType[P];
}[T]; // one or more variants' payloads, by key

function handle<K extends FormFieldType>(type: K, field: FormFieldByType[K]) {
  /* … */
}
```

### Attachment and invoice types

```typescript
// ── Attachments ──
export interface Attachment {
  name: string;
  mimeType?: string; // from the embedded file stream's /Subtype, e.g. "text/xml"
  size: number; // decoded bytes
  relationship?: string; // /AFRelationship: "Source", "Data", "Alternative", "Supplement", ...
  description?: string;
  page?: number; // 1-based page of a /FileAttachment annotation; absent for document-level files
  data: Buffer;
}

// ── Invoice ──
// Every field carries the parsed value, the exact source text, a 0..1 confidence and,
// when it came from a page, the span it was read from. Embedded XML has no spans.
export interface FieldSpan {
  page: number;
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}
export interface StringField {
  value: string;
  text: string;
  confidence: number;
  span?: FieldSpan;
}
export interface NumberField {
  value: number;
  text: string;
  confidence: number;
  span?: FieldSpan;
}
export interface DateField {
  value: string;
  text: string;
  confidence: number;
  span?: FieldSpan;
} // value is YYYY-MM-DD
export interface MoneyField {
  value: number;
  text: string;
  confidence: number;
  span?: FieldSpan;
  currencyCode?: string;
  currencySymbol?: string;
}
export interface AddressField {
  value: string;
  text: string;
  confidence: number;
  span?: FieldSpan;
  lines: Array<string>;
}

export interface InvoiceItem {
  description?: StringField;
  productCode?: StringField;
  quantity?: NumberField;
  unit?: StringField;
  unitPrice?: MoneyField;
  taxRate?: NumberField;
  tax?: MoneyField;
  discount?: MoneyField;
  date?: DateField;
  amount?: MoneyField;
  span?: FieldSpan;
}
export interface TaxDetail {
  rate?: NumberField;
  amount?: MoneyField;
  base?: MoneyField;
  category?: StringField;
}
// Aircraft maintenance, parts and program invoices identify the asset by airframe and/or engine;
// one invoice may cover several, so `aircraft` is a list.
export interface AircraftReference {
  model?: StringField; // "Challenger 350", "Global 6000", ...
  serialNumber?: StringField; // manufacturer serial number (MSN)
  registration?: StringField; // tail number, "N123AB"
  engineModel?: StringField; // "BR710", "AE 3007", ...
  engineSerialNumber?: StringField; // ESN
  span?: FieldSpan;
}
export interface PaymentDetail {
  iban?: StringField;
  swift?: StringField;
  bankName?: StringField;
  accountNumber?: StringField;
  routingNumber?: StringField;
}

export interface InvoiceFields {
  vendorName?: StringField;
  vendorAddress?: AddressField;
  vendorTaxId?: StringField;
  vendorEmail?: StringField;
  vendorPhone?: StringField;
  vendorWebsite?: StringField;
  customerName?: StringField;
  customerId?: StringField;
  customerAddress?: AddressField;
  customerTaxId?: StringField;
  billingAddress?: AddressField;
  shippingAddress?: AddressField;
  remittanceAddress?: AddressField;
  invoiceId?: StringField;
  invoiceDate?: DateField;
  dueDate?: DateField;
  purchaseOrder?: StringField;
  paymentTerm?: StringField;
  serviceStartDate?: DateField;
  serviceEndDate?: DateField;
  currencyCode?: string;
  subTotal?: MoneyField;
  totalDiscount?: MoneyField;
  totalTax?: MoneyField;
  shippingCost?: MoneyField;
  invoiceTotal?: MoneyField;
  amountDue?: MoneyField;
  previousUnpaidBalance?: MoneyField;
  workOrder?: StringField; // MRO work / shop order the invoice bills against
  aircraft: Array<AircraftReference>;
  taxDetails: Array<TaxDetail>;
  paymentDetails: Array<PaymentDetail>;
  items: Array<InvoiceItem>;
}

export type InvoiceSource = "EmbeddedXml" | "Native" | "Ocr" | "Rendered" | "Mixed" | "None";
export type EInvoiceStandard = "FacturX" | "ZUGFeRD" | "XRechnung" | "UBL" | "Unknown";
export type InvoiceDocumentType = "Invoice" | "CreditNote" | "Unknown";

export interface EmbeddedInvoiceXml {
  filename: string;
  standard: EInvoiceStandard;
  profile?: string; // guideline / customization URN, e.g. "urn:cen.eu:en16931:2017#compliant#urn:factur-x.eu:1p0:basic"
  xml: string;
}

export interface InvoiceResult {
  source: InvoiceSource; // "None" when no extraction path applied
  documentType: InvoiceDocumentType;
  confidence: number; // document-level, 0..1
  fields: InvoiceFields;
  warnings: Array<string>;
  embeddedXml?: EmbeddedInvoiceXml;
}

export interface InvoiceOptions {
  languages?: Array<string>; // default ["en"]; reserved for the layout extractor
  currency?: string; // ISO 4217 hint; reserved
  dateOrder?: "Auto" | "MDY" | "DMY" | "YMD"; // reserved
  decimalSeparator?: "Auto" | "." | ","; // reserved
  preferEmbeddedXml?: boolean; // default true
  includeLayout?: boolean; // reserved
  includeCandidates?: boolean; // reserved
}
```

## Usage

### Standalone functions

#### Extract text per page

```typescript
import { readFileSync } from "fs";

import { extractTextPerPage } from "@d0paminedriven/pdfdown";

const pdf = readFileSync("document.pdf");
const pages = extractTextPerPage(pdf);

for (const { page, text } of pages) {
  console.log(`Page ${page}: ${text.slice(0, 100)}...`);
}
```

#### Extract text per page (async)

```typescript
import { readFile } from "fs/promises";

import { extractTextPerPageAsync } from "@d0paminedriven/pdfdown";

const pdf = await readFile("document.pdf");
const pages = await extractTextPerPageAsync(pdf);

for (const { page, text } of pages) {
  console.log(`Page ${page}: ${text.slice(0, 100)}...`);
}
```

#### Extract structured text with header/footer detection

Splits each page into `header`, `body`, and `footer` sections by detecting repeated lines across pages. Lines that appear at the same position (top or bottom) on >= 60% of pages are classified as headers or footers. Page numbers and other varying digits are normalized during comparison, so "Page 1" and "Page 42" are treated as the same line.

```typescript
import { readFileSync } from "fs";

import { extractStructuredTextPerPage } from "@d0paminedriven/pdfdown";

const pdf = readFileSync("document.pdf");
const pages = extractStructuredTextPerPage(pdf);

for (const { page, header, body, footer } of pages) {
  console.log(`Page ${page}:`);
  if (header) console.log(`  Header: ${header}`);
  console.log(`  Body: ${body.slice(0, 100)}...`);
  if (footer) console.log(`  Footer: ${footer}`);
}
```

#### Extract structured text (async)

```typescript
import { readFile } from "fs/promises";

import { extractStructuredTextPerPageAsync } from "@d0paminedriven/pdfdown";

const pdf = await readFile("document.pdf");
const pages = await extractStructuredTextPerPageAsync(pdf);

// Filter out headers/footers for clean text extraction
const cleanText = pages.map((p) => p.body).join("\n\n");
```

#### Extract images as PNG

```typescript
import { readFileSync } from "fs";

import { extractImagesPerPage } from "@d0paminedriven/pdfdown";

const pdf = readFileSync("document.pdf");
const images = extractImagesPerPage(pdf);

for (const img of images) {
  const dataUrl = `data:image/png;base64,${img.data.toString("base64")}`;
  console.log(`Page ${img.page} image ${img.imageIndex}: ${img.width}x${img.height} ${img.colorSpace}`);
}
```

#### Extract images as PNG (async)

```typescript
import { readFile } from "fs/promises";

import { extractImagesPerPageAsync } from "@d0paminedriven/pdfdown";

const pdf = await readFile("document.pdf");
const images = await extractImagesPerPageAsync(pdf);

for (const img of images) {
  const dataUrl = `data:image/png;base64,${img.data.toString("base64")}`;
  console.log(`Page ${img.page} image ${img.imageIndex}: ${img.width}x${img.height} ${img.colorSpace}`);
}
```

#### Extract annotations

```typescript
import { readFileSync } from "fs";

import { extractAnnotationsPerPage } from "@d0paminedriven/pdfdown";

const pdf = readFileSync("document.pdf");
const annots = extractAnnotationsPerPage(pdf);

for (const annot of annots) {
  if (annot.uri) {
    console.log(`Page ${annot.page}: ${annot.uri}`);
  }
  if (annot.dest) {
    console.log(`Page ${annot.page}: internal link to ${annot.dest}`);
  }
}
```

#### Extract annotations (async)

```typescript
import { readFile } from "fs/promises";

import { extractAnnotationsPerPageAsync } from "@d0paminedriven/pdfdown";

const pdf = await readFile("document.pdf");
const annots = await extractAnnotationsPerPageAsync(pdf);

const links = annots.filter((a) => a.uri);
console.log(`Found ${links.length} external links across ${new Set(links.map((a) => a.page)).size} pages`);
```

#### Attachments and embedded e-invoices

`attachments()` lists every embedded file: the `/Names /EmbeddedFiles` tree, PDF/A-3 `/AF` associated files, and `/FileAttachment` annotations, each decoded once. `extractInvoice()` turns an invoice PDF into typed fields with confidence and provenance. In this version the data comes from embedded Factur-X, ZUGFeRD, XRechnung (CII or UBL) or plain UBL XML, which is exact, so every field reports confidence 1 and no page span. PDFs without such XML return `source: "None"` and a warning; layout-based extraction for those arrives in a later release behind the same result shape.

```typescript
import { readFile } from "fs/promises";

import { attachments, extractInvoiceAsync } from "@d0paminedriven/pdfdown";

const buf = await readFile("invoice.pdf");

for (const a of attachments(buf)) {
  console.log(a.name, a.mimeType, a.relationship ?? "-", `${a.size} bytes`, a.page ? `(page ${a.page})` : "");
}

const invoice = await extractInvoiceAsync(buf);
if (invoice.source === "EmbeddedXml") {
  const { fields: f } = invoice;
  console.log(`${f.vendorName?.value} -> ${f.customerName?.value}`);
  console.log(`${f.invoiceId?.value} dated ${f.invoiceDate?.value}, due ${f.dueDate?.value}`);
  console.log(`total ${f.invoiceTotal?.text} ${f.currencyCode}, ${f.items.length} line item(s)`);
  console.log(`standard ${invoice.embeddedXml?.standard}, profile ${invoice.embeddedXml?.profile}`);
} else {
  console.warn(invoice.warnings.join("\n"));
}
```

Money fields expose both `value` (a JS number) and `text` (the exact printed string). Use `text` when cents must survive.

#### Get PDF metadata

```typescript
import { readFileSync } from "fs";

import { pdfMetadata } from "@d0paminedriven/pdfdown";

const pdf = readFileSync("document.pdf");
const meta = pdfMetadata(pdf);

console.log(`v${meta.version}, ${meta.pageCount} pages, linearized: ${meta.isLinearized}`);
```

#### Get PDF metadata (async)

```typescript
import { readFile } from "fs/promises";

import { pdfMetadataAsync } from "@d0paminedriven/pdfdown";

const pdf = await readFile("document.pdf");
const meta = await pdfMetadataAsync(pdf);

console.log(`v${meta.version}, ${meta.pageCount} pages, linearized: ${meta.isLinearized}`);
```

#### Page bounding boxes

`pageBoxes` on `PdfMeta` and `PdfDocument` returns deduplicated page dimensions. Uniform PDFs (all pages the same size) return a single entry. Mixed-size PDFs return one entry per distinct geometry — the dominant (most frequent) entry has `pages` absent, while non-dominant entries list their specific page numbers.

```typescript
import { readFileSync } from "fs";

import { pdfMetadata } from "@d0paminedriven/pdfdown";

const meta = pdfMetadata(readFileSync("document.pdf"));

if (meta.pageBoxes.length === 1) {
  const box = meta.pageBoxes[0];
  console.log(`Uniform: ${box.width}x${box.height} ${box.boxType} (${box.pageCount} pages)`);
} else {
  for (const box of meta.pageBoxes) {
    const scope = box.pages ? `pages ${box.pages.join(", ")}` : "all other pages";
    console.log(`${box.width}x${box.height} ${box.boxType} — ${scope}`);
  }
}
```

#### Extract form fields per page

Read-only. One entry per widget placement, sorted by page, then top-to-bottom, left-to-right. Encrypted PDFs with an empty user password are decrypted on load.

```typescript
import { readFileSync } from "fs";

import { extractFormFieldsPerPage, pdfMetadata } from "@d0paminedriven/pdfdown";

const pdf = readFileSync("application.pdf");

for (const field of extractFormFieldsPerPage(pdf)) {
  switch (field.type) {
    case "Text":
      console.log(`p${field.page} ${field.name} = ${field.value ?? ""}`);
      break;
    case "Checkbox":
      console.log(`p${field.page} ${field.name} ${field.checked ? "[x]" : "[ ]"}`);
      break;
    case "Radio":
      if (field.selected) console.log(`p${field.page} ${field.name} -> ${field.exportValue}`);
      break;
    default:
      console.log(`p${field.page} ${field.type} ${field.name}`);
  }
}

// Group placements back into logical fields
const byField = Map.groupBy(extractFormFieldsPerPage(pdf), (f) => f.fieldObjectId);

// Document-level facts: fields without a widget, truncation, warnings
const form = pdfMetadata(pdf).form;
if (form?.unplacedFields.length)
  console.log(
    "hidden fields:",
    form.unplacedFields.map((u) => u.name)
  );
if (form?.truncated || form?.warnings.length) console.log("partial extraction:", form.warnings);
```

`pdfDocument()` carries the same data as `formFields`, `totalFormFields`, `formFieldPages` and `form`, mirroring `images` / `totalImages` / `imagePages`.

### `PdfDown` class

The class-based API parses the PDF once in the constructor. Sync methods reuse the parsed document directly (zero re-parsing). Async methods share the parsed document across libuv worker threads via `Arc` — no data copying, no re-parsing.

```typescript
import { readFile } from "fs/promises";

import { PdfDown } from "@d0paminedriven/pdfdown";

const pdf = new PdfDown(await readFile("document.pdf"));

// Sync — reuses the already-parsed document
const text = pdf.textPerPage();
const images = pdf.imagesPerPage();
const annots = pdf.annotationsPerPage();
const structured = pdf.structuredText();
const meta = pdf.metadata();

// Async — shares parsed document via Arc across worker threads
const [asyncText, asyncImages, asyncAnnots, asyncStructured] = await Promise.all([
  pdf.textPerPageAsync(),
  pdf.imagesPerPageAsync(),
  pdf.annotationsPerPageAsync(),
  pdf.structuredTextAsync()
]);
```

### Combined: text + images + links for multimodal embeddings

```typescript
import { readFile } from "fs/promises";

import { PdfDown } from "@d0paminedriven/pdfdown";

const pdf = new PdfDown(await readFile("document.pdf"));

const [text, images, annots] = await Promise.all([
  pdf.textPerPageAsync(),
  pdf.imagesPerPageAsync(),
  pdf.annotationsPerPageAsync()
]);

// Group images and links by page
const imagesByPage = Map.groupBy(images, (img) => img.page);
const linksByPage = Map.groupBy(
  annots.filter((a) => a.uri),
  (a) => a.page
);

// Build per-page payloads for multimodal embeddings
for (const { page, text: pageText } of text) {
  const payload = {
    page,
    text: pageText,
    images: (imagesByPage.get(page) ?? []).map((img) => ({
      dataUrl: `data:image/png;base64,${img.data.toString("base64")}`,
      width: img.width,
      height: img.height
    })),
    links: (linksByPage.get(page) ?? []).map((a) => a.uri)
  };
  // Send payload to Voyage AI, pgvector, etc.
}
```

### Combined: structured text for clean content pipelines

```typescript
import { readFile } from "fs/promises";

import { PdfDown } from "@d0paminedriven/pdfdown";

const pdf = new PdfDown(await readFile("document.pdf"));
const pages = await pdf.structuredTextAsync();

// Strip headers/footers for clean RAG ingestion
const chunks = pages
  .map(({ page, body }) => ({
    page,
    content: body.trim()
  }))
  .filter((c) => c.content.length > 0);
```

### Positioned text: words and lines with boxes

`extractTextPerPage` returns text in content-stream order, which is whatever order the generator happened to paint it in (a header drawn last comes out last). `layoutPerPage` interprets the content stream instead: it tracks the graphics and text state, measures each glyph with the font's `/Widths` (or built-in Standard 14 metrics), groups glyphs into words and words into baseline-clustered lines, and returns everything top-to-bottom, left-to-right with boxes in points. Form XObjects are entered through their `/Matrix`, and image XObjects report where they were painted so a scan's OCR words can be mapped back onto the page.

```typescript
import { readFileSync } from "fs";

import { layoutPerPage } from "@d0paminedriven/pdfdown";

const [page] = layoutPerPage(readFileSync("invoice.pdf"));

// Reading order, regardless of how the generator ordered the stream.
for (const line of page.lines) {
  console.log(`${line.y1.toFixed(0).padStart(4)}  ${line.text}`);
}

// Key/value anchoring: the value to the right of a label on the same line.
const labelled = (label: string) => {
  const anchor = page.words.find((w) => w.text === label);
  if (!anchor) return undefined;
  return page.words
    .filter((w) => w.line === anchor.line && w.x0 > anchor.x1)
    .map((w) => w.text)
    .join(" ");
};
console.log(labelled("Invoice"), labelled("Total"));

// Column detection: words whose left edge lines up with a header cell.
const amountHeader = page.words.find((w) => w.text === "Amount" && w.bold);
const amounts = amountHeader
  ? page.words.filter((w) => Math.abs(w.x0 - amountHeader.x0) < 2 && w.y1 < amountHeader.y0)
  : [];
```

Runs in the same order of time as `extractTextPerPage` (single-digit milliseconds for a typical invoice page). For scanned pages use `extractLayoutWithOcrPerPage` from the OCR package, which fills the same shape from Tesseract word boxes: a page is OCR'd when its native text is under `minTextLength` or covers under 2% of a page that is mostly image or heavy with paths, native words stay on such a hybrid page, every page's coordinates share one frame (`contentRotation`), and raster orientation is settled by Tesseract's orientation detector before any blind retry. The OCR package README describes the cascade.

## Workbooks

`PdfDown` opens `.xlsx` workbooks as well as PDFs, in both packages and in the WASI build, with no native dependency: a workbook is a zip of XML, read in pure Rust through the package relationships (never by conventional part names). Hand pdfdown any buffer: `detectFormat` (or `detectFormatAsync`, since naming a zip reads its package directory) says what it is, `format` says what a document was opened as, and a method that reads the other kind fails with an `unsupported operation` error rather than returning a wrong answer: sync methods throw it, async methods reject with it. Nothing is evaluated: formulas keep their cached results, and no cell is ever written.

```typescript
import { PdfDown, detectFormat } from "@d0paminedriven/pdfdown";

detectFormat(buf); // { format: "Pdf" } | { format: "Xlsx" } | { format: "Unknown", detail: "Word document package" }

const doc = await PdfDown.openAsync(buf); // or new PdfDown(buf); either parses once
doc.format; // "Pdf" | "Xlsx"

const sheets = doc.spreadsheetCells(); // every visible sheet's non-empty cells, typed
const pages = doc.layout(); // one logical PageLayout per visible sheet, source "Spreadsheet"
doc.textPerPage(); // throws: unsupported operation: textPerPage reads a PDF; this document is a workbook
```

### Cells

`spreadsheetCells(opts?)` returns one `SheetCells` per selected sheet: `sheet`, `index` (0-based position in the workbook), `hidden`, `dimension?` (the declared range, which a producer may overstate), `printArea?`, `merges` (A1 ranges), `warnings` and `cells`. A cell is non-empty when it holds a value, an error, or a formula (even with an empty cache); a merely styled cell is not listed.

`SheetCell` is a flat discriminated union on `type`, so it narrows on one field and composes with the `UTR` helper:

```typescript
export type SheetCell =
  | { type: "Text"; value: string; ...common }
  | { type: "Number"; value: number; ...common } // the typed value; `text` is the display text under the cell's format
  | { type: "Boolean"; value: boolean; ...common }
  | { type: "Error"; value: string; ...common } // "#DIV/0!", "#N/A", ...
  | { type: "Date"; value: string; ...common } // ISO date, with the time when the format shows one; the serial stays in `raw`
  | { type: "Blank"; ...common }; // a formula cell with no cached result

// ...common on every variant:
//   ref: string;             // "C3"
//   row: number; col: number; // 0-based
//   text: string;            // display text: the format applied to the value, or the stored text
//   raw: string;             // the stored spelling (the serial for dates, "1" for TRUE)
//   format?: string;         // the number format code the cell's style selects; absent for General
//   formatSupported: boolean;// false when this release does not render that code (then text === raw)
//   formula?: string;        // the expression stored on this cell
//   sharedFormula?: string;  // the master cell's ref when this cell follows a shared formula
//   cached: boolean;         // a result was stored for this cell
//   merge?: string;          // the range this cell anchors, e.g. "A1:F1"
//   hidden: boolean;         // in a hidden row or column
```

Cell type comes before number format: a shared string under a date format is still `Text`. Serial 60 in the 1900 date system is the leap day Excel never had and stays a `Number` with a warning; the 1904 date system is honoured.

### Logical pages

`layout(opts?)` on a workbook (and the standalone `layoutFromXlsx`) yields one `PageLayout` per selected sheet with `source: "Spreadsheet"`, `sheet` set, `rotate` and `contentRotation` 0, `images` empty and every word carrying `cell`. Pages are numbered from 1 in selection order. The page is the print area when the sheet declares one, else the bounding box of its non-empty cells; `width` is the sum of the visible column widths and `height` the sum of the visible row heights, in points, y up from the bottom-left like every page. Column widths follow the SpreadsheetML conversion from the stored character count to pixels with Calibri 11 as the Normal font (maximum digit width 7 px at 96 dpi), then 72/96; a default column is 48 pt and a default row 15 pt.

Each cell's display text is placed in its rectangle (a merged range is one rectangle) by the style's alignment: General puts numbers and dates at the right edge, booleans and errors in the middle and text at the left; explicit left, centre and right are honoured, indent adds one em per level, and vertical alignment is bottom unless the style says otherwise. The text extent is an estimate, half an em per character at the cell's font size, so `x0`/`x1` are approximate while rows and columns are exact; explicit newlines make fragments, `wrapText` wraps at the cell width with the same estimate (the page warns that wraps are estimated), and unwrapped text overflows its cell and is clipped to the page. Words are the fragments split on whitespace, with `fontName`, `fontSize`, `bold` and `italic` from the cell font, `symbolic` false and no `confidence`; they go through the same line clusterer as PDF words. Drawings, charts and images are not projected (a warning counts them).

`invoice()` on a workbook returns `source: "None"` with a warning: the layout-based extractor does not read spreadsheet pages yet, and the cells are available from `spreadsheetCells`.

### Options

`SpreadsheetOptions`, accepted by `spreadsheetCells`, `layout` and `layoutFromXlsx` (a PDF ignores them):

- `sheets?: string[]`: sheet names to include, in workbook order; default every visible sheet.
- `hiddenSheets?: boolean`: include hidden sheets (default `false`).
- `hiddenCells?: boolean`: cells in hidden rows and columns. For `spreadsheetCells` the default is `true` (listed with `hidden: true`); for `layout` the default is `false` (zero extent and omitted, with a warning), and `true` gives them their real extents back.

### Number formats

Display text follows the cell's format code, parsed as sections and tokens, with en-US separators and symbol placement only where the code leaves a choice. Rendered in this release: General (integers without a point, otherwise up to 11 significant digits, and scientific notation such as `1E+15` or `1.5E-12` at the extremes); fixed decimals with and without grouping (`0`, `0.00`, `#,##0`, `#,##0.00`) and zero padding (`00000`); percent (`0%`, `0.00%`); quoted and escaped literals anywhere, such as a unit suffix (`#,##0.00"/hr"`); currency and accounting codes with their negative, zero and text sections, parentheses, `_x` padding (one space) and `*x` fill (dropped), colour tags dropped; date and time codes from the built-ins 14 to 22 and custom codes using `y`, `m`, `d`, `h`, `s`, `AM/PM`, where `m` means minutes next to `h` or `s`; the text section `@`. Not rendered (the cell keeps `text === raw`, `formatSupported: false`, and the sheet warns once with the count): elapsed durations (`[h]:mm:ss`), scientific notation, fractions, conditional sections, locale directives (`[$-409]`), era and calendar tokens, codes longer than 255 characters, and built-in format ids this release has no spelling for (a warning names the ids). A file's own `numFmt` for an id wins over the built-in table. Text taller than its row is clipped at the cell's bottom edge, as Excel shows it, with a page warning.

### Unsupported inputs and limits

`detectFormat` names what an input is when it is not a PDF or a workbook, and opening it fails with the same detail: `unsupported format: macro-enabled workbook (not supported in this release)` (`.xlsm`), `template workbook (…)` (`.xltx`), `Strict OOXML workbook (…)`, `Word document package`, `PowerPoint presentation package`, `zip package that is not an Office document`, `OLE container (legacy .xls or an encrypted Office file)`, and `TIFF image`, `JPEG image`, `PNG image` or `RTF document` for files served under a misleading extension. A buffer that is neither a PDF header nor a zip is handed to the PDF parser, whose error names it.

Limits, enforced while reading: input at most 64 MiB compressed, at most 10,000 zip entries, each part at most 64 MiB decompressed and all parts at most 256 MiB, at most 2,000,000 cells and 200,000 merged ranges per workbook; exceeding one throws `limit exceeded: …`. A part that is not well-formed XML throws `malformed workbook: …`; a malformed cell reference, shared-string index or merge range inside an otherwise sound sheet is skipped with a warning instead.

## Measuring on your own corpus

Two scripts run against any local directory of PDFs and `.xlsx` workbooks and print aggregates only; nothing from the corpus is committed, and nothing from it is printed unless you ask. The directory comes from `PDFDOWN_INVOICE_CORPUS` (default: `ocr/__test__/fixtures/private`, which is gitignored) and may be grouped one folder deep. A file's basename is its `id` (the corpus these were built on names files by a six-digit invoice id), and every per-document row in both outputs carries it.

```bash
PDFDOWN_INVOICE_CORPUS=/path/to/pdfs yarn corpus:profile                    # writes <root>/profile.json
PDFDOWN_INVOICE_CORPUS=/path/to/pdfs yarn invoice:eval --ids critical.txt   # writes <root>/eval.json
```

`corpus:profile` runs `pdfMetadata`, `layoutPerPage`, `attachments` and `extractInvoice` once per PDF and prints the headline (parse failures, native layout cost, native / scanned / mixed, pages per document, page sizes, `/Rotate`, AcroForms, attachments and embedded XML), the producer families, the template clusters (a fonts fingerprint and a header fingerprint with digits masked, and how many header clusters cover 50/80/90/95% of the documents) and the page kinds (image-only, a native stamp over a scan, no glyphs and no images, sparse native). Each workbook is opened through `spreadsheetCells` and `layoutFromXlsx` and the Workbooks table follows: how many opened, sheets, cells and merged ranges, cell types, formulas, how many distinct number-format codes appear and how many cells this release does not render, logical pages and their cost. A workbook the reader refuses is a failure row carrying its id, like a PDF.

`invoice:eval` scores `extractInvoice` against an `<id>.expected.json` beside each `<id>.pdf`, skipping and counting the PDFs without one. Header fields are exact matches after trimming, case-folding and whitespace collapsing (dates as `YYYY-MM-DD`, amounts to the cent); items are matched for precision and recall on `(productCode, amount)` when both sides carry a code, else on `(description, amount)`; aircraft on their serial numbers; totals reconcile when the extraction's items add up to its own net total within 1%. Every row is reported twice: document-weighted (each document counts one) and amount-weighted (each counts the magnitude of its expected `invoiceTotal`, so a credit note weighs what it credits; a document without one counts 0 and is reported under `missingAmounts`). A row's amount total is printed only when it sums three or more documents, so no single invoice's total reaches the terminal by default. Rows: all documents, the scanned and mixed subsets, each template cluster from `profile.json` when it exists, and the `--ids` slice (one id per line; the harness does not know why those ids matter).

Expected files use pdfdown's own `InvoiceFields` names, values only. A key you leave out is not scored; a key or value of the wrong type is reported and dropped, never thrown on:

```json
{
  "invoiceId": "INV-2026-0042",
  "invoiceDate": "2026-11-01",
  "invoiceTotal": 833,
  "amountDue": 833,
  "purchaseOrder": "PO-7788",
  "documentType": "Invoice",
  "vendorName": "Acme Aviation Parts GmbH",
  "customerName": "Jet Support Services, Inc.",
  "shippingAddress": { "lines": ["1 Hangar Road", "Anytown"] },
  "items": [
    {
      "productCode": "HPS-220",
      "description": "Hydraulic pump seal kit",
      "quantity": 4,
      "unitPrice": 125,
      "amount": 500
    },
    { "description": "Inspection labor", "quantity": 2.5, "unitPrice": 80, "amount": 200 }
  ],
  "aircraft": [{ "serialNumber": "20123" }]
}
```

Both JSON outputs stay beside the PDFs. `--verbose` names, on the terminal only, the files whose basename is not a six-digit id, whose expected file was partly dropped, or that failed.

## Parallelism

All extraction functions use [rayon](https://github.com/rayon-rs/rayon) for automatic multi-core parallelism. Per-page text extraction, image decoding, and annotation parsing run concurrently across CPU cores. The `pdfDocument` / `document()` call additionally runs text, image, and annotation extraction concurrently via nested `rayon::join`. This is internal to the native addon — the Node.js API surface is unchanged.

OCR runs on a **dedicated capped thread pool** (default 4 threads, configurable via `maxThreads`) to prevent oversubscription when running inside a libuv worker thread, and the OCR package pins Tesseract's own OpenMP to one thread per worker (`OMP_THREAD_LIMIT=1` unless you set it) so those workers do not thrash each other. Non-OCR extraction uses the global rayon pool.

### Tuning Node.js worker threads

Node's libuv thread pool defaults to 4 threads. If you're calling multiple async extraction functions concurrently, you may want to increase it:

```bash
UV_THREADPOOL_SIZE=8 node app.js
```

## Supported formats

| Filter      | Description                | Handling                           |
| ----------- | -------------------------- | ---------------------------------- |
| DCTDecode   | JPEG-compressed images     | Decoded and re-encoded as PNG      |
| JPXDecode   | JPEG 2000 images           | Decoded and re-encoded as PNG      |
| FlateDecode | Zlib-compressed raw pixels | Decompressed, reconstructed as PNG |
| None        | Uncompressed raw pixels    | Reconstructed as PNG               |

| ColorSpace | Channels                   |
| ---------- | -------------------------- |
| DeviceRGB  | 3                          |
| DeviceGray | 1                          |
| DeviceCMYK | 4 (converted to RGB)       |
| ICCBased   | Inferred from /N parameter |

8-bit and 16-bit BitsPerComponent are both supported (16-bit downscaled to 8-bit for PNG output).

## How it works

**Layout** (`layoutPerPage`): a content-stream interpreter tracks `q`/`Q`, `cm`, the text object and text state (`BT`/`ET`, `Tm`, `Td`, `TD`, `T*`, `TL`, `Tf`, `Tc`, `Tw`, `Tz`, `Ts`) and the show operators (`Tj`, `TJ`, `'`, `"`), placing each glyph with the text rendering matrix. Advances come from `/Widths` (simple fonts), `/W` + `/DW` (Type0 composite fonts, two-byte codes), `/FontMatrix` (Type3) or Standard 14 metrics when the file omits widths; ascent/descent from the font descriptor. Glyphs become words at space glyphs, `Tw` gaps, `TJ` adjustments of 200/1000 em or more, baseline changes and horizontal gaps over 0.25 em; words become lines by baseline clustering within half the median word height. Form XObjects are recursed with a depth cap; images are recorded as the unit square under the CTM at `Do`.

Built with [lopdf](https://github.com/J-F-Liu/lopdf) (pure Rust PDF parser), [image](https://github.com/image-rs/image) (PNG/JPEG encoding), [hayro-jpeg2000](https://crates.io/crates/hayro-jpeg2000) (JPEG 2000 decoding), and [rayon](https://github.com/rayon-rs/rayon) (data parallelism). Compiled to a native Node.js addon via [napi-rs](https://napi.rs) with prebuilt binaries for:

- macOS (x64, ARM64)
- Windows (x64, ia32, ARM64)
- Linux glibc (x64, ARM64, ARMv7)
- Linux musl (x64, ARM64)
- FreeBSD (x64)
- Android (ARM64, ARMv7)
- WASI

## License

MIT
