# Invoice corpus

Shared test corpus for the invoice work (design: `notes/2026-11-01.md` §7, §9). Tracked in git
so the same files are available on every machine. Only publicly published vendor samples go
here; real invoices from work are never committed (they live under the gitignored
`ocr/__test__/fixtures/private`).

| File                                           | Producer                 | Pages | Text   | Notes                                                                                                                                                       |
| ---------------------------------------------- | ------------------------ | ----- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Downloadable-PDF-Invoices-Add-On-Samples.pdf` | WHMCS (TCPDF 6.0)        | 4     | native | US dates (`06/23/2018`), `USD $50.00` amounts, Sub Total / Credit / Total, Transactions table. Carries an empty `/AcroForm` (no fields): phase 1 edge case. |
| `wordpress-pdf-invoice-plugin-sample.pdf`      | Sliced Invoices (Quartz) | 1     | native | Label column (`Invoice Number`, `Order Number`, `Invoice Date`, `Due Date`, `Total Due`), From/To blocks, one line item, Tax + Total.                       |

Source: public sample PDFs found via web search on 2026-11-02. Neither file has embedded
e-invoice XML or scanned pages; for OCR coverage render them with `render: "Always"`.
