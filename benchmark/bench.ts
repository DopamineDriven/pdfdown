import { Bench } from 'tinybench'

import {
  extractImagesPerPage,
  extractImagesPerPageAsync,
  extractTextPerPage,
  extractTextPerPageAsync,
  pdfMetadata,
  pdfMetadataAsync,
} from '../index.js'

const PDF_URL =
  'https://assets.aicoalesce.com/upload/nrr6h4r4480f6kviycyo1zhf/1767335919790-Candy-Flipping-Claudtullus-Part-I.pdf'

const res = await fetch(PDF_URL)
if (!res.ok) throw new Error(`Failed to fetch PDF: ${res.status} ${res.statusText}`)
const pdf = Buffer.from(await res.arrayBuffer())

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
