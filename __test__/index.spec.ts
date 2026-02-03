import test from 'ava'

import {
  extractAnnotationsPerPage,
  extractAnnotationsPerPageAsync,
  extractImagesPerPage,
  extractImagesPerPageAsync,
  extractStructuredTextPerPage,
  extractStructuredTextPerPageAsync,
  pdfDocument,
  pdfDocumentAsync,
  PdfDown,
} from '../index'

const EXPECTED_IMAGE_COUNT = 13
const EXPECTED_PAGES = [2, 4, 8, 24, 25, 35, 47, 62, 63, 64, 71]
const PNG_MAGIC = [0x89, 0x50, 0x4e, 0x47]

const PDF_URL =
  'https://assets-dev.aicoalesce.com/upload/nrr6h4r4480f6kviycyo1zhf/1765025311330-Candy-Flipping-Claudtullus-Pt-I.pdf'

const PDF_TWO_URL =
  'https://assets.aicoalesce.com/upload/nrr6h4r4480f6kviycyo1zhf/1761414231832-Building-for-Production_-A-Revised-Playbook-for-Enterprise-Ready-Generative-AI-Solutions_MA-MANDAL.pdf'
const PDF_URL_THREE =
  'https://assets.aicoalesce.com/upload/nrr6h4r4480f6kviycyo1zhf/1767766786190-Lollaclaudplooza-Pt-I.pdf'

const fetcher = async (target: string) => await fetch(target)
const res = await fetcher(PDF_URL)
const annotsRes = await fetcher(PDF_TWO_URL)
const docRes = await fetcher(PDF_URL_THREE)
if (!res.ok) throw new Error(`Failed to fetch PDF: ${res.status} ${res.statusText}`)
if (!annotsRes.ok) throw new Error(`Failed to fetch PDF 2: ${annotsRes.status} ${annotsRes.statusText}`)
if (!docRes.ok) throw new Error(`Failed to fetch PDF 3: ${docRes.status} ${docRes.statusText}`)
const pdf = Buffer.from(await res.arrayBuffer())
const pdf2 = Buffer.from(await annotsRes.arrayBuffer())
const pdf3 = Buffer.from(await docRes.arrayBuffer())
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

// ── PdfDocument comprehensive extraction tests -- targeting "Lollaclaudplooza Pt I"  ─────────────────────────────────

const pdfDown3 = new PdfDown(pdf3)

test('pdfDocument (sync standalone) — returns comprehensive extraction with valid structure', (t) => {
  const doc = pdfDocument(pdf3)

  t.is(typeof doc.version, 'string', 'version should be a string')
  t.is(typeof doc.isLinearized, 'boolean', 'isLinearized should be a boolean')
  t.is(typeof doc.pageCount, 'number', 'pageCount should be a number')
  t.true(doc.pageCount > 0, 'pageCount should be > 0')

  t.log(`version: ${doc.version}`)
  t.log(`isLinearized: ${doc.isLinearized}`)
  t.log(`pageCount: ${doc.pageCount}`)
  t.log(`creator: ${doc.creator ?? '(none)'}`)
  t.log(`producer: ${doc.producer ?? '(none)'}`)
  t.log(`creationDate: ${doc.creationDate ?? '(none)'}`)
  t.log(`modificationDate: ${doc.modificationDate ?? '(none)'}`)

  // Text
  t.true(Array.isArray(doc.text), 'text should be an array')
  t.is(doc.text.length, doc.pageCount, 'text array length should match pageCount')
  for (const { page, text } of doc.text) {
    t.is(typeof page, 'number', 'text page should be a number')
    t.is(typeof text, 'string', 'text content should be a string')
  }

  // Images
  t.true(Array.isArray(doc.images), 'images should be an array')
  t.is(doc.totalImages, doc.images.length, 'totalImages should match images array length')
  t.log(`totalImages: ${doc.totalImages}`)
  t.log(`imagePages: [${doc.imagePages.join(', ')}]`)

  const imagePagesFromImages = [...new Set(doc.images.map((img) => img.page))].sort((a, b) => a - b)
  t.deepEqual(doc.imagePages, imagePagesFromImages, 'imagePages should match unique pages from images array')

  for (const img of doc.images) {
    const header = img.data.subarray(0, 4)
    t.deepEqual([...header], PNG_MAGIC, `image on page ${img.page} (index ${img.imageIndex}) is not a valid PNG`)
  }

  // Annotations
  t.true(Array.isArray(doc.annotations), 'annotations should be an array')
  t.is(doc.totalAnnotations, doc.annotations.length, 'totalAnnotations should match annotations array length')
  t.log(`totalAnnotations: ${doc.totalAnnotations}`)
  t.log(`annotationPages: [${doc.annotationPages.join(', ')}]`)

  const annotPagesFromAnnots = [...new Set(doc.annotations.map((a) => a.page))].sort((a, b) => a - b)
  t.deepEqual(
    doc.annotationPages,
    annotPagesFromAnnots,
    'annotationPages should match unique pages from annotations array',
  )

  for (const annot of doc.annotations) {
    t.is(typeof annot.page, 'number', 'annotation page should be a number')
    t.is(typeof annot.subtype, 'string', 'annotation subtype should be a string')
    t.true(Array.isArray(annot.rect), 'annotation rect should be an array')
  }
})

test('pdfDocumentAsync — returns same results as sync', async (t) => {
  const sync = pdfDocument(pdf3)
  const async_ = await pdfDocumentAsync(pdf3)

  t.is(async_.version, sync.version, 'version should match')
  t.is(async_.isLinearized, sync.isLinearized, 'isLinearized should match')
  t.is(async_.pageCount, sync.pageCount, 'pageCount should match')
  t.is(async_.creator ?? null, sync.creator ?? null, 'creator should match')
  t.is(async_.producer ?? null, sync.producer ?? null, 'producer should match')
  t.is(async_.creationDate ?? null, sync.creationDate ?? null, 'creationDate should match')
  t.is(async_.modificationDate ?? null, sync.modificationDate ?? null, 'modificationDate should match')
  t.is(async_.totalImages, sync.totalImages, 'totalImages should match')
  t.is(async_.totalAnnotations, sync.totalAnnotations, 'totalAnnotations should match')
  t.deepEqual(async_.imagePages, sync.imagePages, 'imagePages should match')
  t.deepEqual(async_.annotationPages, sync.annotationPages, 'annotationPages should match')
  t.is(async_.text.length, sync.text.length, 'text length should match')
  t.is(async_.images.length, sync.images.length, 'images length should match')
  t.is(async_.annotations.length, sync.annotations.length, 'annotations length should match')
})

test('PdfDown.document (sync method) — matches standalone pdfDocument', (t) => {
  const standalone = pdfDocument(pdf3)
  const classResult = pdfDown3.document()

  t.is(classResult.version, standalone.version, 'version should match')
  t.is(classResult.pageCount, standalone.pageCount, 'pageCount should match')
  t.is(classResult.totalImages, standalone.totalImages, 'totalImages should match')
  t.is(classResult.totalAnnotations, standalone.totalAnnotations, 'totalAnnotations should match')
  t.deepEqual(classResult.imagePages, standalone.imagePages, 'imagePages should match')
  t.deepEqual(classResult.annotationPages, standalone.annotationPages, 'annotationPages should match')
  t.is(classResult.text.length, standalone.text.length, 'text length should match')
  t.is(classResult.images.length, standalone.images.length, 'images length should match')
  t.is(classResult.annotations.length, standalone.annotations.length, 'annotations length should match')
  t.is(classResult.creator ?? null, standalone.creator ?? null, 'creator should match')
  t.is(classResult.producer ?? null, standalone.producer ?? null, 'producer should match')
  t.is(classResult.creationDate ?? null, standalone.creationDate ?? null, 'creationDate should match')
  t.is(classResult.modificationDate ?? null, standalone.modificationDate ?? null, 'modificationDate should match')
})

test('PdfDown.documentAsync (async method) — matches standalone pdfDocument', async (t) => {
  const standalone = pdfDocument(pdf3)
  const classResult = await pdfDown3.documentAsync()

  t.is(classResult.version, standalone.version, 'version should match')
  t.is(classResult.pageCount, standalone.pageCount, 'pageCount should match')
  t.is(classResult.totalImages, standalone.totalImages, 'totalImages should match')
  t.is(classResult.totalAnnotations, standalone.totalAnnotations, 'totalAnnotations should match')
  t.deepEqual(classResult.imagePages, standalone.imagePages, 'imagePages should match')
  t.deepEqual(classResult.annotationPages, standalone.annotationPages, 'annotationPages should match')
  t.is(classResult.text.length, standalone.text.length, 'text length should match')
  t.is(classResult.images.length, standalone.images.length, 'images length should match')
  t.is(classResult.annotations.length, standalone.annotations.length, 'annotations length should match')
  t.is(classResult.creator ?? null, standalone.creator ?? null, 'creator should match')
  t.is(classResult.producer ?? null, standalone.producer ?? null, 'producer should match')
  t.is(classResult.creationDate ?? null, standalone.creationDate ?? null, 'creationDate should match')
  t.is(classResult.modificationDate ?? null, standalone.modificationDate ?? null, 'modificationDate should match')
})

// ── Structured text (header/footer detection) tests ─────────────────────────

test('extractStructuredTextPerPage (sync) — returns structured text with valid structure', (t) => {
  const pages = extractStructuredTextPerPage(pdf3)

  t.true(Array.isArray(pages), 'should return an array')
  t.true(pages.length > 0, 'should have at least one page')

  for (const p of pages) {
    t.is(typeof p.page, 'number', 'page should be a number')
    t.is(typeof p.header, 'string', 'header should be a string')
    t.is(typeof p.body, 'string', 'body should be a string')
    t.is(typeof p.footer, 'string', 'footer should be a string')
  }

  // Most pages with text should have non-empty body
  const pagesWithBody = pages.filter((p) => p.body.length > 0)
  t.true(pagesWithBody.length > 0, 'at least some pages should have body text')

  t.log(`Pages: ${pages.length}`)
  t.log(`Pages with headers: ${pages.filter((p) => p.header.length > 0).length}`)
  t.log(`Pages with footers: ${pages.filter((p) => p.footer.length > 0).length}`)
})

test('extractStructuredTextPerPageAsync — returns same results as sync', async (t) => {
  const sync = extractStructuredTextPerPage(pdf3)
  const async_ = await extractStructuredTextPerPageAsync(pdf3)

  t.is(async_.length, sync.length, 'page count should match')

  for (let i = 0; i < sync.length; i++) {
    t.is(async_[i].page, sync[i].page, `page number should match at index ${i}`)
    t.is(async_[i].header, sync[i].header, `header should match at page ${sync[i].page}`)
    t.is(async_[i].body, sync[i].body, `body should match at page ${sync[i].page}`)
    t.is(async_[i].footer, sync[i].footer, `footer should match at page ${sync[i].page}`)
  }
})

test('PdfDown.structuredText (sync method) — matches standalone function', (t) => {
  const standalone = extractStructuredTextPerPage(pdf3)
  const classResult = pdfDown3.structuredText()

  t.is(classResult.length, standalone.length, 'page count should match')

  for (let i = 0; i < standalone.length; i++) {
    t.is(classResult[i].page, standalone[i].page)
    t.is(classResult[i].header, standalone[i].header)
    t.is(classResult[i].body, standalone[i].body)
    t.is(classResult[i].footer, standalone[i].footer)
  }
})

test('PdfDown.structuredTextAsync (async method) — matches standalone function', async (t) => {
  const standalone = extractStructuredTextPerPage(pdf3)
  const classResult = await pdfDown3.structuredTextAsync()

  t.is(classResult.length, standalone.length, 'page count should match')

  for (let i = 0; i < standalone.length; i++) {
    t.is(classResult[i].page, standalone[i].page)
    t.is(classResult[i].header, standalone[i].header)
    t.is(classResult[i].body, standalone[i].body)
    t.is(classResult[i].footer, standalone[i].footer)
  }
})
