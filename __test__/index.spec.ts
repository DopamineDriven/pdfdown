import test from 'ava'

import {
  extractAnnotationsPerPage,
  extractAnnotationsPerPageAsync,
  extractImagesPerPage,
  extractImagesPerPageAsync,
  PdfDown,
} from '../index'

const EXPECTED_IMAGE_COUNT = 13
const EXPECTED_PAGES = [2, 4, 8, 24, 25, 35, 47, 62, 63, 64, 71]
const PNG_MAGIC = [0x89, 0x50, 0x4e, 0x47]

const PDF_URL =
  'https://assets-dev.aicoalesce.com/upload/nrr6h4r4480f6kviycyo1zhf/1765025311330-Candy-Flipping-Claudtullus-Pt-I.pdf'

const PDF_TWO_URL =
  'https://assets.aicoalesce.com/upload/nrr6h4r4480f6kviycyo1zhf/1761414231832-Building-for-Production_-A-Revised-Playbook-for-Enterprise-Ready-Generative-AI-Solutions_MA-MANDAL.pdf'
const fetcher = async (target: string) => await fetch(target)
const res = await fetcher(PDF_URL)
const annotsRes = await fetcher(PDF_TWO_URL)
if (!res.ok) throw new Error(`Failed to fetch PDF: ${res.status} ${res.statusText}`)
const pdf = Buffer.from(await res.arrayBuffer())
const pdf2 = Buffer.from(await annotsRes.arrayBuffer())
test('extractImagesPerPage (sync) — returns 13 images from Candy Flipping Claudtullus Pt I', async (t) => {
  const buf = pdf
  const images = extractImagesPerPage(buf)

  t.is(images.length, EXPECTED_IMAGE_COUNT, `expected ${EXPECTED_IMAGE_COUNT} images, got ${images.length}`)

  const pages = [...new Set(images.map((img) => img.page))].sort((a, b) => a - b)
  t.deepEqual(pages, EXPECTED_PAGES, `expected pages ${EXPECTED_PAGES}, got ${pages}`)

  for (const img of images) {
    const header = img.data.subarray(0, 4)
    t.deepEqual([...header], PNG_MAGIC, `image on page ${img.page} (index ${img.imageIndex}) is not a valid PNG`)
  }
})

test('extractImagesPerPageAsync — returns 13 images from Candy Flipping Claudtullus Pt I', async (t) => {
  const buf = pdf
  const images = await extractImagesPerPageAsync(buf)

  t.is(images.length, EXPECTED_IMAGE_COUNT, `expected ${EXPECTED_IMAGE_COUNT} images, got ${images.length}`)

  const pages = [...new Set(images.map((img) => img.page))].sort((a, b) => a - b)
  t.deepEqual(pages, EXPECTED_PAGES, `expected pages ${EXPECTED_PAGES}, got ${pages}`)

  for (const img of images) {
    const header = img.data.subarray(0, 4)
    t.deepEqual([...header], PNG_MAGIC, `image on page ${img.page} (index ${img.imageIndex}) is not a valid PNG`)
  }
})

const pdfDown = new PdfDown(pdf)

test('PdfDown.imagesPerPage (sync method) — returns 13 images from Candy Flipping Claudtullus Pt I', async (t) => {
  const images = pdfDown.imagesPerPage()

  t.is(images.length, EXPECTED_IMAGE_COUNT, `expected ${EXPECTED_IMAGE_COUNT} images, got ${images.length}`)

  const pages = [...new Set(images.map((img) => img.page))].sort((a, b) => a - b)
  t.deepEqual(pages, EXPECTED_PAGES, `expected pages ${EXPECTED_PAGES}, got ${pages}`)

  for (const img of images) {
    const header = img.data.subarray(0, 4)
    t.deepEqual([...header], PNG_MAGIC, `image on page ${img.page} (index ${img.imageIndex}) is not a valid PNG`)
  }
})

test('PdfDown.imagesPerPageAsync (async method) — returns 13 images from Candy Flipping Claudtullus Pt I', async (t) => {
  const images = await pdfDown.imagesPerPageAsync()

  t.is(images.length, EXPECTED_IMAGE_COUNT, `expected ${EXPECTED_IMAGE_COUNT} images, got ${images.length}`)

  const pages = [...new Set(images.map((img) => img.page))].sort((a, b) => a - b)
  t.deepEqual(pages, EXPECTED_PAGES, `expected pages ${EXPECTED_PAGES}, got ${pages}`)

  for (const img of images) {
    const header = img.data.subarray(0, 4)
    t.deepEqual([...header], PNG_MAGIC, `image on page ${img.page} (index ${img.imageIndex}) is not a valid PNG`)
  }
})

// ── Annotation extraction tests -- targeting "Building for Production: A Revised Playbook for Enterprise Ready Generative AI Solutions"  ─────────────────────────────────

test('extractAnnotationsPerPage (sync) — returns annotations with valid structure', (t) => {
  const annots = extractAnnotationsPerPage(pdf2)

  t.true(Array.isArray(annots), 'should return an array')
  t.log(`Total annotations: ${annots.length}`)

  // Group by subtype for a summary
  const bySubtype = new Map<string, number>()
  for (const annot of annots) {
    bySubtype.set(annot.subtype, (bySubtype.get(annot.subtype) ?? 0) + 1)
  }
  for (const [subtype, count] of bySubtype) {
    t.log(`  ${subtype}: ${count}`)
  }

  // Log all link annotations with URIs
  const links = annots.filter((a) => a.uri)
  t.log(`\nAnnotations with URIs: ${links.length}`)
  for (const link of links) {
    t.log(`  page ${link.page}: ${link.uri}`)
  }

  // Log annotations with internal destinations
  const dests = annots.filter((a) => a.dest)
  if (dests.length > 0) {
    t.log(`\nAnnotations with internal destinations: ${dests.length}`)
    for (const d of dests) {
      t.log(`  page ${d.page}: ${d.dest}`)
    }
  }

  // Group by page for distribution overview
  const byPage = new Map<number, number>()
  for (const annot of annots) {
    byPage.set(annot.page, (byPage.get(annot.page) ?? 0) + 1)
  }
  t.log(`\nAnnotations per page:`)
  for (const [page, count] of [...byPage].sort(([a], [b]) => a - b)) {
    t.log(`  page ${page}: ${count}`)
  }

  for (const annot of annots) {
    t.is(typeof annot.page, 'number', 'page should be a number')
    t.is(typeof annot.subtype, 'string', 'subtype should be a string')
    t.true(Array.isArray(annot.rect), 'rect should be an array')
    t.true(
      annot.rect.length === 0 || annot.rect.length === 4,
      `rect should have 0 or 4 elements, got ${annot.rect.length}`,
    )
  }
})

test('extractAnnotationsPerPageAsync — returns same results as sync', async (t) => {
  const sync = extractAnnotationsPerPage(pdf2)
  const async_ = await extractAnnotationsPerPageAsync(pdf2)

  t.is(async_.length, sync.length, `async returned ${async_.length} annotations, sync returned ${sync.length}`)

  for (let i = 0; i < sync.length; i++) {
    t.is(async_[i].page, sync[i].page)
    t.is(async_[i].subtype, sync[i].subtype)
    t.is(async_[i].uri ?? null, sync[i].uri ?? null)
  }
})
const pdfDown2 = new PdfDown(pdf2)
test('PdfDown.annotationsPerPage (sync method) — matches standalone function', (t) => {
  const standalone = extractAnnotationsPerPage(pdf2)
  const classResult = pdfDown2.annotationsPerPage()

  t.is(classResult.length, standalone.length, 'class and standalone should return same count')

  for (let i = 0; i < standalone.length; i++) {
    t.is(classResult[i].page, standalone[i].page)
    t.is(classResult[i].subtype, standalone[i].subtype)
    t.is(classResult[i].uri ?? null, standalone[i].uri ?? null)
  }
})

test('PdfDown.annotationsPerPageAsync (async method) — matches standalone function', async (t) => {
  const standalone = extractAnnotationsPerPage(pdf2)
  const classResult = await pdfDown2.annotationsPerPageAsync()

  t.is(classResult.length, standalone.length, 'class async and standalone should return same count')

  for (let i = 0; i < standalone.length; i++) {
    t.is(classResult[i].page, standalone[i].page)
    t.is(classResult[i].subtype, standalone[i].subtype)
    t.is(classResult[i].uri ?? null, standalone[i].uri ?? null)
  }
})
