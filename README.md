# `@d0paminedriven/pdfdown`

![CI](https://github.com/DopamineDriven/pdfdown/workflows/CI/badge.svg)

Rust-powered PDF extraction for Node.js via [napi-rs](https://napi.rs). Extracts per-page text, images (as PNG), annotations (links, destinations), structured text (with header/footer detection), and metadata from PDF buffers. All extraction is parallelized with [rayon](https://github.com/rayon-rs/rayon) for multi-core performance.

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
export declare function extractTextPerPage(buffer: Buffer): Array<PageText>
export declare function extractImagesPerPage(buffer: Buffer): Array<PageImage>
export declare function extractAnnotationsPerPage(buffer: Buffer): Array<PageAnnotation>
export declare function extractStructuredTextPerPage(buffer: Buffer): Array<StructuredPageText>
export declare function pdfMetadata(buffer: Buffer): PdfMeta
export declare function pdfDocument(buffer: Buffer): PdfDocument
```

#### Async (libuv thread pool)

Each sync function has an async counterpart that runs on the libuv thread pool, keeping the main thread free.

```typescript
export declare function extractTextPerPageAsync(buffer: Buffer): Promise<Array<PageText>>
export declare function extractImagesPerPageAsync(buffer: Buffer): Promise<Array<PageImage>>
export declare function extractAnnotationsPerPageAsync(buffer: Buffer): Promise<Array<PageAnnotation>>
export declare function extractStructuredTextPerPageAsync(buffer: Buffer): Promise<Array<StructuredPageText>>
export declare function pdfMetadataAsync(buffer: Buffer): Promise<PdfMeta>
export declare function pdfDocumentAsync(buffer: Buffer): Promise<PdfDocument>
```

### `PdfDown` class

Parse once, call many. The constructor parses the PDF document once and reuses it across all sync method calls. Async methods share the parsed document across worker threads via `Arc` (atomic reference counting) with zero re-parsing overhead.

```typescript
export declare class PdfDown {
  constructor(buffer: Buffer)
  textPerPage(): Array<PageText>
  imagesPerPage(): Array<PageImage>
  annotationsPerPage(): Array<PageAnnotation>
  structuredText(): Array<StructuredPageText>
  metadata(): PdfMeta
  document(): PdfDocument
  textPerPageAsync(): Promise<Array<PageText>>
  imagesPerPageAsync(): Promise<Array<PageImage>>
  annotationsPerPageAsync(): Promise<Array<PageAnnotation>>
  structuredTextAsync(): Promise<Array<StructuredPageText>>
  metadataAsync(): Promise<PdfMeta>
  documentAsync(): Promise<PdfDocument>
}
```

### Types

```typescript
export interface PageText {
  page: number
  text: string
}

export interface StructuredPageText {
  page: number
  header: string
  body: string
  footer: string
}

export interface PageImage {
  page: number
  imageIndex: number
  width: number
  height: number
  data: Buffer // PNG-encoded bytes
  colorSpace: string
  bitsPerComponent: number
  filter: string
  xobjectName: string
  objectId: string
}

export interface PageAnnotation {
  page: number
  subtype: string // "Link", "Text", "Highlight", etc.
  rect: Array<number> // [x1, y1, x2, y2] bounding box
  uri?: string // external link URL
  dest?: string // internal named destination
  content?: string // tooltip / alt text
}

export const enum BoxType {
  CropBox = 'CropBox',
  MediaBox = 'MediaBox',
  Unknown = 'Unknown',
}

export interface PageBox {
  pageCount: number // number of pages with these dimensions
  left: number
  bottom: number
  right: number
  top: number
  width: number
  height: number
  boxType: BoxType
  pages?: Array<number> // only present on non-dominant entries
}

export interface PdfMeta {
  pageCount: number
  version: string
  isLinearized: boolean
  creator?: string
  producer?: string
  creationDate?: string
  modificationDate?: string
  pageBoxes: Array<PageBox>
}

export interface PdfDocument {
  version: string
  isLinearized: boolean
  pageCount: number
  creator?: string
  producer?: string
  creationDate?: string
  modificationDate?: string
  pageBoxes: Array<PageBox>
  totalImages: number
  totalAnnotations: number
  imagePages: Array<number>
  annotationPages: Array<number>
  text: Array<PageText>
  structuredText: Array<StructuredPageText>
  images: Array<PageImage>
  annotations: Array<PageAnnotation>
}
```

## Usage

### Standalone functions

#### Extract text per page

```typescript
import { readFileSync } from 'fs'
import { extractTextPerPage } from '@d0paminedriven/pdfdown'

const pdf = readFileSync('document.pdf')
const pages = extractTextPerPage(pdf)

for (const { page, text } of pages) {
  console.log(`Page ${page}: ${text.slice(0, 100)}...`)
}
```

#### Extract text per page (async)

```typescript
import { readFile } from 'fs/promises'
import { extractTextPerPageAsync } from '@d0paminedriven/pdfdown'

const pdf = await readFile('document.pdf')
const pages = await extractTextPerPageAsync(pdf)

for (const { page, text } of pages) {
  console.log(`Page ${page}: ${text.slice(0, 100)}...`)
}
```

#### Extract structured text with header/footer detection

Splits each page into `header`, `body`, and `footer` sections by detecting repeated lines across pages. Lines that appear at the same position (top or bottom) on >= 60% of pages are classified as headers or footers. Page numbers and other varying digits are normalized during comparison, so "Page 1" and "Page 42" are treated as the same line.

```typescript
import { readFileSync } from 'fs'
import { extractStructuredTextPerPage } from '@d0paminedriven/pdfdown'

const pdf = readFileSync('document.pdf')
const pages = extractStructuredTextPerPage(pdf)

for (const { page, header, body, footer } of pages) {
  console.log(`Page ${page}:`)
  if (header) console.log(`  Header: ${header}`)
  console.log(`  Body: ${body.slice(0, 100)}...`)
  if (footer) console.log(`  Footer: ${footer}`)
}
```

#### Extract structured text (async)

```typescript
import { readFile } from 'fs/promises'
import { extractStructuredTextPerPageAsync } from '@d0paminedriven/pdfdown'

const pdf = await readFile('document.pdf')
const pages = await extractStructuredTextPerPageAsync(pdf)

// Filter out headers/footers for clean text extraction
const cleanText = pages.map((p) => p.body).join('\n\n')
```

#### Extract images as PNG

```typescript
import { readFileSync } from 'fs'
import { extractImagesPerPage } from '@d0paminedriven/pdfdown'

const pdf = readFileSync('document.pdf')
const images = extractImagesPerPage(pdf)

for (const img of images) {
  const dataUrl = `data:image/png;base64,${img.data.toString('base64')}`
  console.log(`Page ${img.page} image ${img.imageIndex}: ${img.width}x${img.height} ${img.colorSpace}`)
}
```

#### Extract images as PNG (async)

```typescript
import { readFile } from 'fs/promises'
import { extractImagesPerPageAsync } from '@d0paminedriven/pdfdown'

const pdf = await readFile('document.pdf')
const images = await extractImagesPerPageAsync(pdf)

for (const img of images) {
  const dataUrl = `data:image/png;base64,${img.data.toString('base64')}`
  console.log(`Page ${img.page} image ${img.imageIndex}: ${img.width}x${img.height} ${img.colorSpace}`)
}
```

#### Extract annotations

```typescript
import { readFileSync } from 'fs'
import { extractAnnotationsPerPage } from '@d0paminedriven/pdfdown'

const pdf = readFileSync('document.pdf')
const annots = extractAnnotationsPerPage(pdf)

for (const annot of annots) {
  if (annot.uri) {
    console.log(`Page ${annot.page}: ${annot.uri}`)
  }
  if (annot.dest) {
    console.log(`Page ${annot.page}: internal link to ${annot.dest}`)
  }
}
```

#### Extract annotations (async)

```typescript
import { readFile } from 'fs/promises'
import { extractAnnotationsPerPageAsync } from '@d0paminedriven/pdfdown'

const pdf = await readFile('document.pdf')
const annots = await extractAnnotationsPerPageAsync(pdf)

const links = annots.filter((a) => a.uri)
console.log(`Found ${links.length} external links across ${new Set(links.map((a) => a.page)).size} pages`)
```

#### Get PDF metadata

```typescript
import { readFileSync } from 'fs'
import { pdfMetadata } from '@d0paminedriven/pdfdown'

const pdf = readFileSync('document.pdf')
const meta = pdfMetadata(pdf)

console.log(`v${meta.version}, ${meta.pageCount} pages, linearized: ${meta.isLinearized}`)
```

#### Get PDF metadata (async)

```typescript
import { readFile } from 'fs/promises'
import { pdfMetadataAsync } from '@d0paminedriven/pdfdown'

const pdf = await readFile('document.pdf')
const meta = await pdfMetadataAsync(pdf)

console.log(`v${meta.version}, ${meta.pageCount} pages, linearized: ${meta.isLinearized}`)
```

#### Page bounding boxes

`pageBoxes` on `PdfMeta` and `PdfDocument` returns deduplicated page dimensions. Uniform PDFs (all pages the same size) return a single entry. Mixed-size PDFs return one entry per distinct geometry — the dominant (most frequent) entry has `pages` absent, while non-dominant entries list their specific page numbers.

```typescript
import { readFileSync } from 'fs'
import { pdfMetadata } from '@d0paminedriven/pdfdown'

const meta = pdfMetadata(readFileSync('document.pdf'))

if (meta.pageBoxes.length === 1) {
  const box = meta.pageBoxes[0]
  console.log(`Uniform: ${box.width}x${box.height} ${box.boxType} (${box.pageCount} pages)`)
} else {
  for (const box of meta.pageBoxes) {
    const scope = box.pages ? `pages ${box.pages.join(', ')}` : 'all other pages'
    console.log(`${box.width}x${box.height} ${box.boxType} — ${scope}`)
  }
}
```

### `PdfDown` class

The class-based API parses the PDF once in the constructor. Sync methods reuse the parsed document directly (zero re-parsing). Async methods share the parsed document across libuv worker threads via `Arc` — no data copying, no re-parsing.

```typescript
import { readFile } from 'fs/promises'
import { PdfDown } from '@d0paminedriven/pdfdown'

const pdf = new PdfDown(await readFile('document.pdf'))

// Sync — reuses the already-parsed document
const text = pdf.textPerPage()
const images = pdf.imagesPerPage()
const annots = pdf.annotationsPerPage()
const structured = pdf.structuredText()
const meta = pdf.metadata()

// Async — shares parsed document via Arc across worker threads
const [asyncText, asyncImages, asyncAnnots, asyncStructured] = await Promise.all([
  pdf.textPerPageAsync(),
  pdf.imagesPerPageAsync(),
  pdf.annotationsPerPageAsync(),
  pdf.structuredTextAsync(),
])
```

### Combined: text + images + links for multimodal embeddings

```typescript
import { readFile } from 'fs/promises'
import { PdfDown } from '@d0paminedriven/pdfdown'

const pdf = new PdfDown(await readFile('document.pdf'))

const [text, images, annots] = await Promise.all([
  pdf.textPerPageAsync(),
  pdf.imagesPerPageAsync(),
  pdf.annotationsPerPageAsync(),
])

// Group images and links by page
const imagesByPage = Map.groupBy(images, (img) => img.page)
const linksByPage = Map.groupBy(
  annots.filter((a) => a.uri),
  (a) => a.page,
)

// Build per-page payloads for multimodal embeddings
for (const { page, text: pageText } of text) {
  const payload = {
    page,
    text: pageText,
    images: (imagesByPage.get(page) ?? []).map((img) => ({
      dataUrl: `data:image/png;base64,${img.data.toString('base64')}`,
      width: img.width,
      height: img.height,
    })),
    links: (linksByPage.get(page) ?? []).map((a) => a.uri),
  }
  // Send payload to Voyage AI, pgvector, etc.
}
```

### Combined: structured text for clean content pipelines

```typescript
import { readFile } from 'fs/promises'
import { PdfDown } from '@d0paminedriven/pdfdown'

const pdf = new PdfDown(await readFile('document.pdf'))
const pages = await pdf.structuredTextAsync()

// Strip headers/footers for clean RAG ingestion
const chunks = pages.map(({ page, body }) => ({
  page,
  content: body.trim(),
})).filter((c) => c.content.length > 0)
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
