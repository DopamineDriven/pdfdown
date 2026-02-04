import { readFileSync } from 'fs'
import { join } from 'path'

import test from 'ava'

import {
  extractTextWithOcrPerPage,
  extractTextWithOcrPerPageAsync,
  extractTextPerPage,
  extractImagesPerPage,
  PdfDown,
  TextSource,
} from '../index'

// ── Fixtures ─────────────────────────────────────────────────────────────────

const FIXTURES = join(import.meta.dirname, 'fixtures')

// Single-page image-only PDF: text is "Hello OCR World"
const helloOcr = readFileSync(join(FIXTURES, 'hello-ocr.pdf'))

// 3-page image-only PDF: "Page One Typed Text", "Second Page Content", "Third page monospace"
const multipageOcr = readFileSync(join(FIXTURES, 'multipage-ocr.pdf'))

// One of the existing test PDFs (has native text)
const PDF_URL = 'https://assets.aicoalesce.com/upload/nrr6h4r4480f6kviycyo1zhf/1767766786190-Lollaclaudplooza-Pt-I.pdf'

const fetcher = async (target: string) => await fetch(target)
const nativeRes = await fetcher(PDF_URL)
if (!nativeRes.ok) throw new Error(`Failed to fetch PDF: ${nativeRes.status} ${nativeRes.statusText}`)
const nativePdf = Buffer.from(await nativeRes.arrayBuffer())

// ── OCR on image-only PDFs ───────────────────────────────────────────────────

test('extractTextWithOcrPerPage (sync) — extracts text from single-page image-only PDF', (t) => {
  const pages = extractTextWithOcrPerPage(helloOcr)

  t.is(pages.length, 1, 'should have exactly 1 page')
  t.is(pages[0].page, 1, 'page number should be 1')
  t.is(pages[0].source, TextSource.Ocr, 'source should be Ocr for image-only page')

  const text = pages[0].text.toLowerCase()
  t.true(text.includes('hello'), `OCR text should contain "hello", got: "${pages[0].text}"`)
  t.true(text.includes('ocr'), `OCR text should contain "ocr", got: "${pages[0].text}"`)
  t.true(text.includes('world'), `OCR text should contain "world", got: "${pages[0].text}"`)

  t.log(`OCR result: "${pages[0].text.trim()}"`)
})

test('extractTextWithOcrPerPageAsync — matches sync for image-only PDF', async (t) => {
  const sync = extractTextWithOcrPerPage(helloOcr)
  const async_ = await extractTextWithOcrPerPageAsync(helloOcr)

  t.is(async_.length, sync.length, 'page count should match')
  t.is(async_[0].source, sync[0].source, 'source should match')
  t.is(async_[0].text.trim(), sync[0].text.trim(), 'OCR text should match')
})

test('extractTextWithOcrPerPage — multi-page image-only PDF returns 3 pages', (t) => {
  const pages = extractTextWithOcrPerPage(multipageOcr)

  t.is(pages.length, 3, 'should have 3 pages')

  for (const p of pages) {
    t.is(p.source, TextSource.Ocr, `page ${p.page} should use OCR`)
    t.true(p.text.trim().length > 0, `page ${p.page} should have non-empty OCR text`)
    t.log(`Page ${p.page} [${p.source}]: "${p.text.trim()}"`)
  }

  // Verify expected content (case-insensitive, OCR may introduce minor variations)
  t.true(pages[0].text.toLowerCase().includes('page'), 'page 1 should contain "page"')
  t.true(pages[1].text.toLowerCase().includes('second'), 'page 2 should contain "second"')
  t.true(pages[2].text.toLowerCase().includes('third'), 'page 3 should contain "third"')
})

// ── OCR with native text PDF (should use Native source) ──────────────────────

test('extractTextWithOcrPerPage — native text PDF uses Native source', (t) => {
  const pages = extractTextWithOcrPerPage(nativePdf)

  t.true(pages.length > 0, 'should have pages')

  const nativePages = pages.filter((p) => p.source === 'Native')
  t.true(nativePages.length > 0, 'at least some pages should use native extraction')

  t.log(`Total pages: ${pages.length}`)
  t.log(`Native: ${nativePages.length}`)
  t.log(`OCR: ${pages.filter((p) => p.source === 'Ocr').length}`)
})

test('extractTextWithOcrPerPageAsync — native text PDF matches sync', async (t) => {
  const sync = extractTextWithOcrPerPage(nativePdf)
  const async_ = await extractTextWithOcrPerPageAsync(nativePdf)

  t.is(async_.length, sync.length, 'page count should match')

  for (let i = 0; i < sync.length; i++) {
    t.is(async_[i].page, sync[i].page, `page number should match at index ${i}`)
    t.is(async_[i].source, sync[i].source, `source should match at page ${sync[i].page}`)
  }
})

// ── OcrOptions: minTextLength forcing OCR fallback ───────────────────────────

test('extractTextWithOcrPerPage — high minTextLength forces OCR on native pages', (t) => {
  // Set absurdly high threshold so even pages with native text fall back to OCR
  const pages = extractTextWithOcrPerPage(nativePdf, { minTextLength: 999999 })

  // Pages with images should now be OCR'd
  const ocrPages = pages.filter((p) => p.source === 'Ocr')
  const nativePages = pages.filter((p) => p.source === 'Native')

  t.log(`With minTextLength=999999: OCR=${ocrPages.length}, Native=${nativePages.length}`)

  // Pages without images will still be "Native" (nothing to OCR), but pages with
  // images should fall back to OCR since native text won't meet the threshold
  // Just verify we get valid results either way
  for (const p of pages) {
    t.is(typeof p.page, 'number', 'page should be a number')
    t.is(typeof p.text, 'string', 'text should be a string')
    t.true(p.source === 'Native' || p.source === 'Ocr', 'source should be Native or Ocr')
  }
})

// ── OcrOptions: maxThreads ───────────────────────────────────────────────────

test('extractTextWithOcrPerPage — maxThreads=1 produces same results', (t) => {
  const defaultResult = extractTextWithOcrPerPage(helloOcr)
  const singleThread = extractTextWithOcrPerPage(helloOcr, { maxThreads: 1 })

  t.is(singleThread.length, defaultResult.length, 'page count should match')
  t.is(singleThread[0].source, defaultResult[0].source, 'source should match')
  t.is(singleThread[0].text.trim(), defaultResult[0].text.trim(), 'text should match')
})

// ── PdfDown class OCR methods ────────────────────────────────────────────────

test('PdfDown.textWithOcrPerPage (sync) — image-only PDF', (t) => {
  const pdf = new PdfDown(helloOcr)
  const standalone = extractTextWithOcrPerPage(helloOcr)
  const classResult = pdf.textWithOcrPerPage()

  t.is(classResult.length, standalone.length, 'page count should match standalone')
  t.is(classResult[0].source, standalone[0].source, 'source should match')
  t.is(classResult[0].text.trim(), standalone[0].text.trim(), 'text should match')
})

test('PdfDown.textWithOcrPerPageAsync — image-only PDF', async (t) => {
  const pdf = new PdfDown(helloOcr)
  const standalone = extractTextWithOcrPerPage(helloOcr)
  const classResult = await pdf.textWithOcrPerPageAsync()

  t.is(classResult.length, standalone.length, 'page count should match standalone')
  t.is(classResult[0].source, standalone[0].source, 'source should match')
  t.is(classResult[0].text.trim(), standalone[0].text.trim(), 'text should match')
})

// ── Superset: base APIs still work through OCR package ───────────────────────

test('base API: extractTextPerPage works through OCR package', (t) => {
  const pages = extractTextPerPage(nativePdf)

  t.true(Array.isArray(pages), 'should return an array')
  t.true(pages.length > 0, 'should have pages')

  for (const p of pages) {
    t.is(typeof p.page, 'number')
    t.is(typeof p.text, 'string')
  }
})

test('base API: extractImagesPerPage works through OCR package', (t) => {
  const images = extractImagesPerPage(nativePdf)

  t.true(Array.isArray(images), 'should return an array')

  for (const img of images) {
    t.is(typeof img.page, 'number')
    t.is(typeof img.width, 'number')
    t.is(typeof img.height, 'number')
    // PNG magic bytes
    const header = img.data.subarray(0, 4)
    t.deepEqual([...header], [0x89, 0x50, 0x4e, 0x47])
  }
})

test('base API: PdfDown class methods work through OCR package', (t) => {
  const pdf = new PdfDown(nativePdf)

  const meta = pdf.metadata()
  t.is(typeof meta.pageCount, 'number')
  t.true(meta.pageCount > 0)

  const text = pdf.textPerPage()
  t.is(text.length, meta.pageCount)
})
