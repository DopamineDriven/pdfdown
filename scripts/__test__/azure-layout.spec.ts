import test from "ava";

import { invoiceFromLayout } from "../../index";
import { azureToLayout } from "../lib/azure-layout";
import type { AzureReadResult, AzureWord } from "../lib/azure-layout";

// An invented letter page in Azure's shape: inches from the top-left, eight-number polygons.
// Two words per line, offsets laid out as Azure does over the page's content string.
function word(content: string, left: number, top: number, offset: number, confidence = 0.98): AzureWord {
  const width = content.length * 0.08;
  const height = 0.14;
  return {
    content,
    polygon: [left, top, left + width, top, left + width, top + height, left, top + height],
    confidence,
    span: { offset, length: content.length }
  };
}

const result: AzureReadResult = {
  pages: [
    {
      pageNumber: 1,
      angle: 0,
      width: 8.5,
      height: 11,
      unit: "inch",
      words: [
        word("INVOICE", 0.7, 0.5, 0),
        word("2048811", 1.5, 0.5, 8),
        word("Invoice", 5.0, 1.0, 16),
        word("Date:", 5.6, 1.0, 24),
        word("11/06/2026", 6.1, 1.0, 30),
        word("Total", 5.0, 9.0, 41),
        word("Due", 5.5, 9.0, 47),
        word("$93.50", 6.5, 9.0, 51)
      ],
      lines: [
        {
          content: "INVOICE 2048811",
          polygon: [0.7, 0.5, 2.2, 0.5, 2.2, 0.64, 0.7, 0.64],
          spans: [{ offset: 0, length: 15 }]
        },
        {
          content: "Invoice Date: 11/06/2026",
          polygon: [5.0, 1.0, 7.0, 1.0, 7.0, 1.14, 5.0, 1.14],
          spans: [{ offset: 16, length: 24 }]
        },
        {
          content: "Total Due $93.50",
          polygon: [5.0, 9.0, 7.0, 9.0, 7.0, 9.14, 5.0, 9.14],
          spans: [{ offset: 41, length: 16 }]
        }
      ]
    }
  ]
};

test("azure pages become pdfdown pages in points with y up and contiguous lines", (t) => {
  const [page] = azureToLayout(result);
  t.is(page.page, 1);
  t.is(page.source, "Ocr");
  t.is(page.width, 612);
  t.is(page.height, 792);
  t.is(page.words.length, 8);
  t.is(page.lines.length, 3);
  const first = page.words[0];
  t.is(first.text, "INVOICE");
  t.true(Math.abs(first.x0 - 0.7 * 72) < 1e-9);
  // 0.5in from the top of an 11in page is 10.36in from the bottom, the box's top edge.
  t.true(Math.abs(first.y1 - (11 - 0.5) * 72) < 1e-9);
  t.true(first.y0 < first.y1);
  t.is(first.confidence, 0.98);
  t.true((first.fontSize ?? 0) > 6 && (first.fontSize ?? 0) < 12);
  let next = 0;
  for (const [i, line] of page.lines.entries()) {
    t.is(line.firstWord, next);
    t.true(page.words.slice(line.firstWord, line.firstWord + line.wordCount).every((w) => w.line === i));
    next += line.wordCount;
  }
  t.is(next, page.words.length);
});

test("the invoice reader reads an azure page through the seam", (t) => {
  const r = invoiceFromLayout(azureToLayout(result));
  t.is(r.source, "Ocr");
  t.is(r.fields.invoiceId?.value, "2048811");
  t.is(r.fields.invoiceDate?.value, "2026-11-06");
  t.is(r.fields.invoiceTotal?.value, 93.5);
});

test("a label and its value in separate azure regions share one baseline line", (t) => {
  // Azure puts "FECHA:" and the date in two regions; pdfdown needs them on one line so the
  // value is the label's right-hand neighbour.
  const columns: AzureReadResult = {
    pages: [
      {
        pageNumber: 1,
        width: 8.5,
        height: 11,
        unit: "inch",
        words: [
          word("FECHA:", 5.0, 1.0, 0),
          word("23/07/2014", 6.5, 1.0, 7),
          word("TOTAL:", 5.0, 9.0, 18),
          word("US$", 6.2, 9.0, 25),
          word("400.00", 6.8, 9.0, 29)
        ],
        lines: [
          { content: "FECHA:", polygon: [5.0, 1.0, 5.5, 1.0, 5.5, 1.14, 5.0, 1.14], spans: [{ offset: 0, length: 6 }] },
          {
            content: "23/07/2014",
            polygon: [6.5, 1.0, 7.3, 1.0, 7.3, 1.14, 6.5, 1.14],
            spans: [{ offset: 7, length: 10 }]
          },
          {
            content: "TOTAL:",
            polygon: [5.0, 9.0, 5.5, 9.0, 5.5, 9.14, 5.0, 9.14],
            spans: [{ offset: 18, length: 6 }]
          },
          { content: "US$", polygon: [6.2, 9.0, 6.4, 9.0, 6.4, 9.14, 6.2, 9.14], spans: [{ offset: 25, length: 3 }] },
          { content: "400.00", polygon: [6.8, 9.0, 7.3, 9.0, 7.3, 9.14, 6.8, 9.14], spans: [{ offset: 29, length: 6 }] }
        ]
      }
    ]
  };
  const [page] = azureToLayout(columns);
  t.is(page.lines.length, 2);
  t.is(page.lines[0].text, "FECHA: 23/07/2014");
  t.is(page.lines[1].text, "TOTAL: US$ 400.00");
  const r = invoiceFromLayout([page]);
  t.is(r.fields.invoiceDate?.value, "2014-07-23");
  t.is(r.fields.invoiceTotal?.value, 400);
  t.is(r.fields.currencyCode, "USD");
});

test("pixel pages are scaled at 96 per inch and odd units are warned about", (t) => {
  const pixel: AzureReadResult = {
    pages: [{ ...result.pages[0], unit: "pixel", width: 816, height: 1056 }]
  };
  const [page] = azureToLayout(pixel);
  t.is(page.width, 612);
  t.is(page.height, 792);
  const odd: AzureReadResult = { pages: [{ ...result.pages[0], unit: "furlong" }] };
  t.true(azureToLayout(odd)[0].warnings[0].includes("furlong"));
});
