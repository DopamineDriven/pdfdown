# `@d0paminedriven/pdfdown-ocr`

Rust-powered PDF extraction for Node.js with Tesseract OCR fallback for image-only pages. A superset of [`@d0paminedriven/pdfdown`](https://www.npmjs.com/package/@d0paminedriven/pdfdown) — includes all base extraction APIs (text, images, annotations, structured text, metadata) plus OCR.

**System requirement:** [Tesseract](https://github.com/tesseract-ocr/tesseract) 5.x must be installed on the host.

## Install

```bash
npm install @d0paminedriven/pdfdown-ocr
```

### Tesseract setup

```bash
# Ubuntu/Debian (22.04 ships tesseract 3.x — use the PPA for 5.x)
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

Verify with `tesseract --version` — you should see 5.x.

## API

This package exports everything from `@d0paminedriven/pdfdown` (text, images, annotations, structured text, metadata — both sync and async), plus the OCR-specific APIs below. See the [base package docs](https://www.npmjs.com/package/@d0paminedriven/pdfdown) for the full base API.

### OCR standalone functions

```typescript
export declare function extractTextWithOcrPerPage(
  buffer: Buffer,
  opts?: OcrOptions,
): Array<OcrPageText>

export declare function extractTextWithOcrPerPageAsync(
  buffer: Buffer,
  opts?: OcrOptions,
): Promise<Array<OcrPageText>>
```

### `PdfDown` class (includes OCR methods)

```typescript
export declare class PdfDown {
  constructor(buffer: Buffer)

  // All base methods (textPerPage, imagesPerPage, annotationsPerPage,
  // structuredText, metadata, document — sync and async variants)

  // OCR methods:
  textWithOcrPerPage(opts?: OcrOptions): Array<OcrPageText>
  textWithOcrPerPageAsync(opts?: OcrOptions): Promise<Array<OcrPageText>>
}
```

### Types

```typescript
export const enum TextSource {
  Native = 'Native',
  Ocr = 'Ocr',
}

export interface OcrPageText {
  page: number
  text: string
  source: TextSource
}

export interface OcrOptions {
  lang?: string        // Tesseract language code, default "eng"
  minTextLength?: number // non-whitespace char threshold before OCR fallback, default 1
  maxThreads?: number  // cap on Rayon threads for OCR parallelism, default 4, clamped to [1, available CPUs]
}
```

## Usage

> **Use the async API for OCR.** The sync variants block the Node.js event loop for the duration of OCR processing, which can be significant for multi-page scanned documents. Prefer `extractTextWithOcrPerPageAsync` / `textWithOcrPerPageAsync` in production.

### Standalone

```typescript
import { readFile } from 'fs/promises'
import { extractTextWithOcrPerPageAsync } from '@d0paminedriven/pdfdown-ocr'

const pdf = await readFile('scanned-document.pdf')
const pages = await extractTextWithOcrPerPageAsync(pdf, { lang: 'eng', minTextLength: 10 })

for (const { page, text, source } of pages) {
  console.log(`Page ${page} [${source}]: ${text.slice(0, 100)}...`)
}
```

### Class-based (parse once, extract many)

```typescript
import { readFile } from 'fs/promises'
import { PdfDown } from '@d0paminedriven/pdfdown-ocr'

const pdf = new PdfDown(await readFile('scanned-document.pdf'))

// OCR text extraction
const pages = await pdf.textWithOcrPerPageAsync({ lang: 'eng', minTextLength: 10 })

// All base methods work too
const images = await pdf.imagesPerPageAsync()
const meta = pdf.metadata()
```

### Combined: OCR text + images for multimodal pipelines

```typescript
import { readFile } from 'fs/promises'
import { PdfDown } from '@d0paminedriven/pdfdown-ocr'

const pdf = new PdfDown(await readFile('scanned-document.pdf'))

const [ocrText, images] = await Promise.all([
  pdf.textWithOcrPerPageAsync({ minTextLength: 10 }),
  pdf.imagesPerPageAsync(),
])

const imagesByPage = Map.groupBy(images, (img) => img.page)

for (const { page, text, source } of ocrText) {
  const pageImages = (imagesByPage.get(page) ?? []).map((img) => ({
    dataUrl: `data:image/png;base64,${img.data.toString('base64')}`,
    width: img.width,
    height: img.height,
  }))
  // Send { page, text, source, images: pageImages } to your embedding pipeline
}
```

## How it works

Pages with native text are extracted directly. When a page yields fewer non-whitespace characters than `minTextLength`, its embedded images are decoded and fed to Tesseract for OCR. Each result is tagged with `source: 'Native'` or `source: 'Ocr'` so you know which path was taken.

OCR runs on a dedicated capped thread pool (default 4 threads, configurable via `maxThreads`) to prevent CPU oversubscription.

## Supported platforms

Prebuilt binaries are provided for:

- macOS (x64, ARM64)
- Linux glibc (x64, ARM64)

## Relationship to `@d0paminedriven/pdfdown`

Same Rust codebase, compiled with the `ocr` Cargo feature flag enabled. This package is a strict superset — you can use it as a drop-in replacement for the base package if you need OCR capabilities.

## License

MIT
