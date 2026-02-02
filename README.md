# `@d0paminedriven/pdfdown`

![CI](https://github.com/DopamineDriven/pdfdown/workflows/CI/badge.svg)

Rust-powered PDF extraction for Node.js via [napi-rs](https://napi.rs). Extracts per-page text, images (as PNG), annotations (links, destinations), and metadata from PDF buffers.

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
export declare function pdfMetadata(buffer: Buffer): PdfMeta
```

#### Async (libuv thread pool)

Each sync function has an async counterpart that runs on the libuv thread pool, keeping the main thread free.

```typescript
export declare function extractTextPerPageAsync(buffer: Buffer): Promise<Array<PageText>>
export declare function extractImagesPerPageAsync(buffer: Buffer): Promise<Array<PageImage>>
export declare function extractAnnotationsPerPageAsync(buffer: Buffer): Promise<Array<PageAnnotation>>
export declare function pdfMetadataAsync(buffer: Buffer): Promise<PdfMeta>
```

### `PdfDown` class

Parse once, call many. The constructor parses the PDF document once and reuses it across all sync method calls. Async methods share the parsed document across worker threads via `Arc` (atomic reference counting) with zero re-parsing overhead.

```typescript
export declare class PdfDown {
  constructor(buffer: Buffer)
  textPerPage(): Array<PageText>
  imagesPerPage(): Array<PageImage>
  annotationsPerPage(): Array<PageAnnotation>
  metadata(): PdfMeta
  textPerPageAsync(): Promise<Array<PageText>>
  imagesPerPageAsync(): Promise<Array<PageImage>>
  annotationsPerPageAsync(): Promise<Array<PageAnnotation>>
  metadataAsync(): Promise<PdfMeta>
}
```

### Types

```typescript
export interface PageText {
  page: number
  text: string
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

export interface PdfMeta {
  pageCount: number
  version: string
  isLinearized: boolean
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
const meta = pdf.metadata()

// Async — shares parsed document via Arc across worker threads
const [asyncText, asyncImages, asyncAnnots] = await Promise.all([
  pdf.textPerPageAsync(),
  pdf.imagesPerPageAsync(),
  pdf.annotationsPerPageAsync(),
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

Built with [lopdf](https://github.com/J-F-Liu/lopdf) (pure Rust PDF parser), [image](https://github.com/image-rs/image) (PNG/JPEG encoding), and [hayro-jpeg2000](https://crates.io/crates/hayro-jpeg2000) (JPEG 2000 decoding). Compiled to a native Node.js addon via [napi-rs](https://napi.rs) with prebuilt binaries for:

- macOS (x64, ARM64)
- Windows (x64, ia32, ARM64)
- Linux glibc (x64, ARM64, ARMv7)
- Linux musl (x64, ARM64)
- FreeBSD (x64)
- Android (ARM64, ARMv7)
- WASI

## License

MIT
