# `@d0paminedriven/pdfdown`

![CI](https://github.com/DopamineDriven/pdfdown/workflows/CI/badge.svg)

Rust-powered PDF extraction for Node.js via [napi-rs](https://napi.rs). Extracts per-page text, images (as PNG), and metadata from PDF buffers.

## Install

```bash
npm install @d0paminedriven/pdfdown
# or
yarn add @d0paminedriven/pdfdown
# or
pnpm add @d0paminedriven/pdfdown
```

## API

### Synchronous

```typescript
export declare function extractTextPerPage(buffer: Buffer): Array<PageText>
export declare function extractImagesPerPage(buffer: Buffer): Array<PageImage>
export declare function pdfMetadata(buffer: Buffer): PdfMeta
```

### Async (libuv thread pool)

Each sync function has an async counterpart that runs on the libuv thread pool, keeping the main thread free.

```typescript
export declare function extractTextPerPageAsync(buffer: Buffer): Promise<Array<PageText>>
export declare function extractImagesPerPageAsync(buffer: Buffer): Promise<Array<PageImage>>
export declare function pdfMetadataAsync(buffer: Buffer): Promise<PdfMeta>
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

export interface PdfMeta {
  pageCount: number
  version: string
  isLinearized: boolean
}
```

## Usage

### Extract text per page

```typescript
import { readFileSync } from 'fs'
import { extractTextPerPage } from '@d0paminedriven/pdfdown'

const pdf = readFileSync('document.pdf')
const pages = extractTextPerPage(pdf)

for (const { page, text } of pages) {
  console.log(`Page ${page}: ${text.slice(0, 100)}...`)
}
```

### Extract text per page (async)

```typescript
import { readFile } from 'fs/promises'
import { extractTextPerPageAsync } from '@d0paminedriven/pdfdown'

const pdf = await readFile('document.pdf')
const pages = await extractTextPerPageAsync(pdf)

for (const { page, text } of pages) {
  console.log(`Page ${page}: ${text.slice(0, 100)}...`)
}
```

### Extract images as PNG

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

### Extract images as PNG (async)

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

### Get PDF metadata

```typescript
import { readFileSync } from 'fs'
import { pdfMetadata } from '@d0paminedriven/pdfdown'

const pdf = readFileSync('document.pdf')
const meta = pdfMetadata(pdf)

console.log(`v${meta.version}, ${meta.pageCount} pages, linearized: ${meta.isLinearized}`)
```

### Get PDF metadata (async)

```typescript
import { readFile } from 'fs/promises'
import { pdfMetadataAsync } from '@d0paminedriven/pdfdown'

const pdf = await readFile('document.pdf')
const meta = await pdfMetadataAsync(pdf)

console.log(`v${meta.version}, ${meta.pageCount} pages, linearized: ${meta.isLinearized}`)
```

### Combined: text + images for multimodal embeddings

```typescript
import { readFile } from 'fs/promises'
import { extractTextPerPageAsync, extractImagesPerPageAsync } from '@d0paminedriven/pdfdown'

const pdf = await readFile('document.pdf')

const [text, images] = await Promise.all([extractTextPerPageAsync(pdf), extractImagesPerPageAsync(pdf)])

// Group images by page
const imagesByPage = new Map<number, typeof images>()
for (const img of images) {
  const arr = imagesByPage.get(img.page) ?? []
  arr.push(img)
  imagesByPage.set(img.page, arr)
}

// Build per-page payloads for multimodal embeddings
for (const { page, text: pageText } of text) {
  const pageImages = imagesByPage.get(page) ?? []
  const payload = {
    page,
    text: pageText,
    images: pageImages.map((img) => ({
      dataUrl: `data:image/png;base64,${img.data.toString('base64')}`,
      width: img.width,
      height: img.height,
    })),
  }
  // Send payload to Voyage AI, pgvector, etc.
}
```

## Supported formats

| Filter      | Description                | Handling                           |
| ----------- | -------------------------- | ---------------------------------- |
| DCTDecode   | JPEG-compressed images     | Decoded and re-encoded as PNG      |
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

Built with [lopdf](https://github.com/J-F-Liu/lopdf) (pure Rust PDF parser) and [image](https://github.com/image-rs/image) (PNG/JPEG encoding). Compiled to a native Node.js addon via [napi-rs](https://napi.rs) with prebuilt binaries for:

- macOS (x64, ARM64)
- Windows (x64, ia32, ARM64)
- Linux glibc (x64, ARM64, ARMv7)
- Linux musl (x64, ARM64)
- FreeBSD (x64)
- Android (ARM64, ARMv7)
- WASI

## License

MIT
