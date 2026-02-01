import test from 'ava'

import { extractImagesPerPage, extractImagesPerPageAsync, PdfDown } from '../index'

const EXPECTED_IMAGE_COUNT = 13
const EXPECTED_PAGES = [2, 4, 8, 24, 25, 35, 47, 62, 63, 64, 71]
const PNG_MAGIC = [0x89, 0x50, 0x4e, 0x47]

const PDF_URL =
  'https://assets-dev.aicoalesce.com/upload/nrr6h4r4480f6kviycyo1zhf/1765025311330-Candy-Flipping-Claudtullus-Pt-I.pdf'

const res = await fetch(PDF_URL)
if (!res.ok) throw new Error(`Failed to fetch PDF: ${res.status} ${res.statusText}`)
const pdf = Buffer.from(await res.arrayBuffer())
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
