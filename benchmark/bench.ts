import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { Bench } from 'tinybench'

import {
  extractImagesPerPage,
  extractImagesPerPageAsync,
  extractTextPerPage,
  extractTextPerPageAsync,
  pdfMetadata,
  pdfMetadataAsync,
} from '../index.js'

const PDF_PATH = resolve(
  '/home/dopaminedriven/personal/d0paminedriven/packages/metadata/src/test/__benchmark__/Candy-Flipping-Claudtullus-Pt-I.pdf',
)

const pdf = readFileSync(PDF_PATH)

const b = new Bench()

b.add('extractTextPerPage (sync)', () => {
  return extractTextPerPage(pdf)
})

b.add('extractTextPerPageAsync', async () => {
  return await extractTextPerPageAsync(pdf)
})

b.add('extractImagesPerPage (sync)', () => {
  return extractImagesPerPage(pdf)
})

b.add('extractImagesPerPageAsync', async () => {
  return await extractImagesPerPageAsync(pdf)
})

b.add('pdfMetadata (sync)', () => {
  return pdfMetadata(pdf)
})

b.add('pdfMetadataAsync', async () => {
  return await pdfMetadataAsync(pdf)
})

b.run().then((_tasks) => {
  console.table(b.table())
})
