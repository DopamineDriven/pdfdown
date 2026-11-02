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
export declare function extractStructuredTextPerPage(buffer: Buffer): Array<StructuredPageText>;
export declare function pdfMetadata(buffer: Buffer): PdfMeta;
export declare function pdfDocument(buffer: Buffer): PdfDocument;
export declare function extractFormFieldsPerPage(buffer: Buffer): Array<PageFormField>;
```

#### Async (libuv thread pool)

Each sync function has an async counterpart that runs on the libuv thread pool, keeping the main thread free.

```typescript
export declare function extractTextPerPageAsync(buffer: Buffer): Promise<Array<PageText>>;
export declare function extractImagesPerPageAsync(buffer: Buffer): Promise<Array<PageImage>>;
export declare function extractAnnotationsPerPageAsync(buffer: Buffer): Promise<Array<PageAnnotation>>;
export declare function attachmentsAsync(buffer: Buffer): Promise<Array<Attachment>>;
export declare function extractInvoiceAsync(buffer: Buffer, opts?: InvoiceOptions): Promise<InvoiceResult>;
export declare function extractStructuredTextPerPageAsync(buffer: Buffer): Promise<Array<StructuredPageText>>;
export declare function pdfMetadataAsync(buffer: Buffer): Promise<PdfMeta>;
export declare function pdfDocumentAsync(buffer: Buffer): Promise<PdfDocument>;
export declare function extractFormFieldsPerPageAsync(buffer: Buffer): Promise<Array<PageFormField>>;
```

### `PdfDown` class

Parse once, call many. The constructor parses the PDF document once and reuses it across all sync method calls. Async methods share the parsed document across worker threads via `Arc` (atomic reference counting) with zero re-parsing overhead.

```typescript
export declare class PdfDown {
  constructor(buffer: Buffer);
  textPerPage(): Array<PageText>;
  imagesPerPage(): Array<PageImage>;
  annotationsPerPage(): Array<PageAnnotation>;
  attachments(): Array<Attachment>;
  invoice(opts?: InvoiceOptions): InvoiceResult;
  structuredText(): Array<StructuredPageText>;
  metadata(): PdfMeta;
  document(): PdfDocument;
  textPerPageAsync(): Promise<Array<PageText>>;
  imagesPerPageAsync(): Promise<Array<PageImage>>;
  annotationsPerPageAsync(): Promise<Array<PageAnnotation>>;
  attachmentsAsync(): Promise<Array<Attachment>>;
  invoiceAsync(opts?: InvoiceOptions): Promise<InvoiceResult>;
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

Every `index.d.ts` starts with a small set of type utilities (`Rm`, `XOR`, `CTR`, `RTC`, `UTR`, `IsExact`, …) and two aliases derived from the generated declarations:

```typescript
export type FormFieldType = PageFormField["type"]; // "Text" | "Checkbox" | …
export type FormFieldByType = UTR<PageFormField, "type", true>; // { Text: {…}, Checkbox: {…}, … } without the discriminant

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

## Parallelism

All extraction functions use [rayon](https://github.com/rayon-rs/rayon) for automatic multi-core parallelism. Per-page text extraction, image decoding, and annotation parsing run concurrently across CPU cores. The `pdfDocument` / `document()` call additionally runs text, image, and annotation extraction concurrently via nested `rayon::join`. This is internal to the native addon — the Node.js API surface is unchanged.

OCR runs on a **dedicated capped thread pool** (default 4 threads, configurable via `maxThreads`) to prevent oversubscription when running inside a libuv worker thread. Non-OCR extraction uses the global rayon pool.

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
