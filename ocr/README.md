# `@d0paminedriven/pdfdown-ocr`

Rust-powered PDF extraction for Node.js with Tesseract OCR fallback for image-only pages. A superset of [`@d0paminedriven/pdfdown`](https://www.npmjs.com/package/@d0paminedriven/pdfdown) -- includes all base extraction APIs (text, images, annotations, structured text, metadata) plus OCR.

**System requirement:** [Tesseract](https://github.com/tesseract-ocr/tesseract) 5.x must be installed on the host.

## Install

```bash
npm install @d0paminedriven/pdfdown-ocr
```

### Tesseract setup

```bash
# Ubuntu/Debian (22.04 ships pre v5 > -- use the PPA for 5.x)
sudo add-apt-repository ppa:alex-p/tesseract-ocr5
sudo apt update
sudo apt install tesseract-ocr tesseract-ocr-eng -y
# Optional: all language packs
# sudo apt install tesseract-ocr-all

# macOS
brew install tesseract

# Arch
sudo pacman -S tesseract tesseract-data-eng
```

Verify with `tesseract --version` -- you should see 5.x.

### Tessdata auto-detection

The package automatically detects the tessdata directory at runtime by parsing the output of `tesseract --list-langs`. The detected path is cached for the lifetime of the process using a `OnceLock<Option<String>>` -- no global environment mutation, fully thread-safe.

**Resolution order:**

1. `TESSDATA_PREFIX` environment variable (if set, used as-is -- no auto-detection runs)
2. Auto-detection via `tesseract --list-langs` (parses the path from `List of available languages in "/path/to/tessdata/"`)
3. Tesseract's compiled-in default (if neither of the above yields a path)

Most users will not need to set `TESSDATA_PREFIX` at all. The auto-detection handles standard installations on Ubuntu (`/usr/share/tesseract-ocr/5/tessdata/`), macOS Homebrew (`/opt/homebrew/share/tessdata/`), Arch, and any other layout where `tesseract` is on `PATH`.

Set `TESSDATA_PREFIX` explicitly only if:

- Tesseract is not on `PATH` but the tessdata directory exists elsewhere
- You want to override the detected path (e.g., pointing to a custom-trained data directory)

```bash
# Override example (not usually needed)
export TESSDATA_PREFIX="/opt/custom/tessdata"
```

## API

This package exports everything from `@d0paminedriven/pdfdown` (text, images, annotations, AcroForm fields, attachments, embedded e-invoice extraction, structured text, metadata -- both sync and async), plus the OCR-specific APIs below. `PdfDocumentOcr` carries the same `form`, `formFields`, `totalFormFields` and `formFieldPages` aggregates as `PdfDocument`; form extraction is read-only and needs neither Tesseract nor PDFium. See the [base package docs](https://www.npmjs.com/package/@d0paminedriven/pdfdown) for the full base API.

### OCR standalone functions

```typescript
// Per-page OCR text extraction
export declare function extractTextWithOcrPerPage(buffer: Buffer, opts?: OcrOptions): Array<OcrPageText>;

export declare function extractTextWithOcrPerPageAsync(buffer: Buffer, opts?: OcrOptions): Promise<Array<OcrPageText>>;

// Full document extraction with OCR text fallback
export declare function pdfDocumentOcr(buffer: Buffer, opts?: OcrOptions): PdfDocumentOcr;

export declare function pdfDocumentOcrAsync(buffer: Buffer, opts?: OcrOptions): Promise<PdfDocumentOcr>;

// Positioned words and lines with the same three-tier cascade
export declare function extractLayoutWithOcrPerPage(buffer: Buffer, opts?: OcrOptions): Array<PageLayout>;

export declare function extractLayoutWithOcrPerPageAsync(buffer: Buffer, opts?: OcrOptions): Promise<Array<PageLayout>>;
```

### `PdfDown` class (includes OCR methods)

```typescript
export declare class PdfDown {
  constructor(buffer: Buffer);

  // ── Base methods ──
  textPerPage(): Array<PageText>;
  textPerPageAsync(): Promise<Array<PageText>>;
  imagesPerPage(): Array<PageImage>;
  imagesPerPageAsync(): Promise<Array<PageImage>>;
  annotationsPerPage(): Array<PageAnnotation>;
  annotationsPerPageAsync(): Promise<Array<PageAnnotation>>;
  attachments(): Array<Attachment>;
  attachmentsAsync(): Promise<Array<Attachment>>;
  invoice(opts?: InvoiceOptions): InvoiceResult;
  invoiceAsync(opts?: InvoiceOptions): Promise<InvoiceResult>;
  structuredText(): Array<StructuredPageText>;
  structuredTextAsync(): Promise<Array<StructuredPageText>>;
  metadata(): PdfMeta;
  metadataAsync(): Promise<PdfMeta>;
  document(): PdfDocument;
  documentAsync(): Promise<PdfDocument>;
  formFieldsPerPage(): Array<PageFormField>;
  formFieldsPerPageAsync(): Promise<Array<PageFormField>>;

  // ── OCR methods ──
  textWithOcrPerPage(opts?: OcrOptions): Array<OcrPageText>;
  textWithOcrPerPageAsync(opts?: OcrOptions): Promise<Array<OcrPageText>>;
  documentOcr(opts?: OcrOptions): PdfDocumentOcr;
  documentOcrAsync(opts?: OcrOptions): Promise<PdfDocumentOcr>;
  layoutWithOcr(opts?: OcrOptions): Array<PageLayout>;
  layoutWithOcrAsync(opts?: OcrOptions): Promise<Array<PageLayout>>;
}
```

### Types

```typescript
export type TextSource = "Native" | "Ocr" | "Rendered";

export interface OcrPageText {
  page: number;
  text: string;
  source: TextSource;
}

export interface OcrStructuredPageText {
  page: number;
  header: string;
  body: string;
  footer: string;
  source: TextSource;
}

export interface OcrOptions {
  lang?: string; // Tesseract language code, default "eng"
  minTextLength?: number; // non-whitespace char threshold before OCR fallback, default 1
  maxThreads?: number; // cap on Rayon threads for OCR parallelism, default 4, clamped to [1, available CPUs]
}

export interface PdfDocumentOcr {
  version: string;
  isLinearized: boolean;
  pageCount: number;
  creator?: string;
  producer?: string;
  creationDate?: string;
  modificationDate?: string;
  totalImages: number;
  totalAnnotations: number;
  imagePages: Array<number>;
  annotationPages: Array<number>;
  text: Array<OcrPageText>;
  structuredText: Array<OcrStructuredPageText>;
  images: Array<PageImage>;
  annotations: Array<PageAnnotation>;
  form?: FormInfo;
  formFields: Array<PageFormField>;
  totalFormFields: number;
  formFieldPages: Array<number>;
}
```

## Usage

> **Use the async API for OCR.** The sync variants block the Node.js event loop for the duration of OCR processing, which can be significant for multi-page scanned documents.

### Standalone

```typescript
import { readFile } from "fs/promises";

import { extractTextWithOcrPerPageAsync } from "@d0paminedriven/pdfdown-ocr";

const pdf = await readFile("scanned-document.pdf");
const pages = await extractTextWithOcrPerPageAsync(pdf, { lang: "eng", minTextLength: 10 });

for (const { page, text, source } of pages) {
  console.log(`Page ${page} [${source}]: ${text.slice(0, 100)}...`);
}
```

### Class-based (parse once, extract many)

```typescript
import { readFile } from "fs/promises";

import { PdfDown } from "@d0paminedriven/pdfdown-ocr";

const pdf = new PdfDown(await readFile("scanned-document.pdf"));

// OCR text extraction
const pages = await pdf.textWithOcrPerPageAsync({ lang: "eng", minTextLength: 10 });

// All base methods work too
const images = await pdf.imagesPerPageAsync();
const meta = pdf.metadata();
```

### Extract everything with OCR in one call

```typescript
import { readFile } from "fs/promises";

import { PdfDown } from "@d0paminedriven/pdfdown-ocr";

const pdf = new PdfDown(await readFile("scanned-document.pdf"));
const result = await pdf.documentOcrAsync({ minTextLength: 10 });

// result.text         — OcrPageText[] (page, text, source per page)
// result.structuredText — OcrStructuredPageText[] (header/body/footer + source per page)
// result.images       — PageImage[] (decoded PNGs with dimensions and color space)
// result.annotations  — PageAnnotation[] (links, destinations, rects)
// result.pageCount, result.version, result.creator, ...
```

### Combined: OCR text + images for multimodal pipelines

```typescript
import { readFile } from "fs/promises";

import { PdfDown } from "@d0paminedriven/pdfdown-ocr";

const pdf = new PdfDown(await readFile("scanned-document.pdf"));

const [ocrText, images] = await Promise.all([
  pdf.textWithOcrPerPageAsync({ minTextLength: 10 }),
  pdf.imagesPerPageAsync()
]);

const imagesByPage = Map.groupBy(images, (img) => img.page);

for (const { page, text, source } of ocrText) {
  const pageImages = (imagesByPage.get(page) ?? []).map((img) => ({
    dataUrl: `data:image/png;base64,${img.data.toString("base64")}`,
    width: img.width,
    height: img.height
  }));
  // Send { page, text, source, images: pageImages } to your embedding pipeline
}
```

### `document()` vs `documentOcr()`

Both methods extract everything from a PDF in a single call. The difference is how text is extracted:

| Method                                 | Text extraction          | Return type      | Use when                                 |
| -------------------------------------- | ------------------------ | ---------------- | ---------------------------------------- |
| `document()` / `documentAsync()`       | Native PDF text only     | `PdfDocument`    | PDF has selectable text                  |
| `documentOcr()` / `documentOcrAsync()` | Native with OCR fallback | `PdfDocumentOcr` | PDF may contain scanned/image-only pages |

`PdfDocumentOcr` uses `OcrPageText` (with `source: 'Native' | 'Ocr'`) and `OcrStructuredPageText` (with header/body/footer split plus source) instead of the base `PageText` and `StructuredPageText` types. Images, annotations, metadata and the read-only AcroForm data (`form`, `formFields`, `totalFormFields`, `formFieldPages`; see the base package docs) are identical in both.

### Positioned text with OCR

`extractLayoutWithOcrPerPage` returns the base package's `PageLayout` shape (words, lines and image placements with boxes in PDF points) for every page, choosing the source per page exactly like the text cascade:

1. **Native** -- glyphs placed from the content stream, with `fontName` / `fontSize` / `bold` on every word.
2. **Ocr** -- the page's embedded raster images are OCR'd and each Tesseract word box is mapped through the image's `Do` placement onto the page. Words carry `confidence` (0..1) instead of font metadata.
3. **Rendered** -- with PDFium, the page is rasterised at `renderDpi` and OCR'd; word boxes are mapped onto the page box. Pages with `/Rotate` get a warning because the boxes are in displayed orientation.

Copier and fax scans (`CCITTFaxDecode`, Group 3 and Group 4) are decoded in pure Rust, so they are read at tier 2 without PDFium. When the upright pass scores a mean Tesseract confidence under 55 (a landscape sheet scanned in portrait comes back as punctuation at 35-45), the raster is OCR'd again turned 90°, 270° and 180° and the best orientation wins. The page then reports `contentRotation`, the turn carrying the most confident OCR text across its images (the tiles of a tiled scan vote together), and every word, line and image placement is expressed in that turned frame, with `width`/`height` swapped for a quarter turn. A warning names each turned image with more than a few words and who decided the turn (the orientation detector or the confidence retry); such an image that reads upright at a different turn than the page is named too, since its words read sideways in the page frame. The same retry applies to `extractTextWithOcrPerPage`.

```typescript
import { readFileSync } from "fs";

import { extractLayoutWithOcrPerPage } from "@d0paminedriven/pdfdown-ocr";

for (const page of extractLayoutWithOcrPerPage(readFileSync("scan.pdf"), { render: "Auto" })) {
  console.log(`page ${page.page} (${page.source})`);
  for (const line of page.lines) {
    const confidence = page.words.slice(line.firstWord, line.firstWord + line.wordCount).map((w) => w.confidence ?? 1);
    console.log(`  ${Math.min(...confidence).toFixed(2)}  ${line.text}`);
  }
}
```

## How it works

1. **Text extraction:** Each page is first attempted with native PDF text extraction. If a page yields fewer non-whitespace characters than `minTextLength`, its embedded images are decoded and fed to Tesseract for OCR. Each result is tagged with `source: 'Native'` or `source: 'Ocr'`.

2. **Structured text:** After text extraction, repeated header/footer lines are detected across pages using frequency analysis (requires 3+ pages). Each page's text is split into `header`, `body`, and `footer` sections. For OCR results, the `source` tag is preserved so you know whether each page's content came from native extraction or OCR.

3. **Parallelism:** OCR runs on a dedicated capped Rayon thread pool (default 4 threads, configurable via `maxThreads`) to prevent CPU oversubscription. Text extraction, image extraction, and annotation extraction run concurrently via `rayon::join` when using `documentOcr` / `documentOcrAsync`. The package entry point also sets `OMP_THREAD_LIMIT=1` before the native module loads, because Ubuntu's libtesseract otherwise starts one OpenMP thread per core inside every recognise call and four workers on an eight-core box thrash it (measured: 7.8 s per page down to 0.6 s). Parallelism therefore comes from `maxThreads`, not from OpenMP; set `OMP_THREAD_LIMIT` or `OMP_NUM_THREADS` yourself before requiring the package to override this.

4. **Tessdata discovery:** On first OCR invocation, the tessdata path is resolved once and cached in a `OnceLock`. The `TESSDATA_PREFIX` environment variable is checked first; if unset, `tesseract --list-langs` is executed and its output is parsed to extract the path. No environment variables are mutated -- the path is passed directly to Tesseract's init function.

## Supported platforms

Prebuilt binaries are provided for:

- macOS (ARM64)
- Linux glibc (x64, ARM64)

Intel macOS is not built: the OCR package links the system Tesseract, and Homebrew no longer
ships Intel bottles, so there is no supported way to obtain the library on that platform.
The base `@d0paminedriven/pdfdown` package still ships an Intel macOS binary.

## Relationship to `@d0paminedriven/pdfdown`

Same Rust codebase, compiled with the `ocr` Cargo feature flag enabled. This package is a strict superset -- you can use it as a drop-in replacement for the base package if you need OCR capabilities.

## License

MIT
