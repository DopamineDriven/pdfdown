import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import test from 'ava'

import { extractImagesPerPage, extractImagesPerPageAsync } from '../index'

const PDF_PATH = resolve(
  '/home/dopaminedriven/personal/d0paminedriven/packages/metadata/src/test/__benchmark__/Candy-Flipping-Claudtullus-Pt-I.pdf',
)
const EXPECTED_IMAGE_COUNT = 13
const EXPECTED_PAGES = [2, 4, 8, 24, 25, 35, 47, 62, 63, 64, 71]
const PNG_MAGIC = [0x89, 0x50, 0x4e, 0x47]

function loadPdf(): Buffer {
  return readFileSync(PDF_PATH)
}

test('extractImagesPerPage (sync) — returns 13 images from Candy Flipping Claudtullus Pt I', (t) => {
  const buf = loadPdf()
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
  const buf = loadPdf()
  const images = await extractImagesPerPageAsync(buf)

  t.is(images.length, EXPECTED_IMAGE_COUNT, `expected ${EXPECTED_IMAGE_COUNT} images, got ${images.length}`)

  const pages = [...new Set(images.map((img) => img.page))].sort((a, b) => a - b)
  t.deepEqual(pages, EXPECTED_PAGES, `expected pages ${EXPECTED_PAGES}, got ${pages}`)

  for (const img of images) {
    const header = img.data.subarray(0, 4)
    t.deepEqual([...header], PNG_MAGIC, `image on page ${img.page} (index ${img.imageIndex}) is not a valid PNG`)
  }
})
