import { readFileSync } from "fs";
import { join } from "path";

import test from "ava";

import {
  capabilities,
  extractLayoutWithOcrPerPage,
  extractLayoutWithOcrPerPageAsync,
  layoutPerPage,
  PdfDown
} from "../index";
import type { PageLayout } from "../index";

const FIXTURES = join(import.meta.dirname, "fixtures");
const helloOcr = readFileSync(join(FIXTURES, "hello-ocr.pdf"));
const multipageOcr = readFileSync(join(FIXTURES, "multipage-ocr.pdf"));
const ascii85DctScan = readFileSync(join(import.meta.dirname, "../../__test__/fixtures/ascii85-dct-scan.pdf"));
const layoutSample = readFileSync(join(import.meta.dirname, "../../__test__/fixtures/layout/layout-sample.pdf"));

const caps = capabilities();

const lineTexts = (page: PageLayout) => page.lines.map((l) => l.text);

const assertOcrWords = (t: { true: (v: boolean, m?: string) => void }, page: PageLayout) => {
  for (const w of page.words) {
    t.true(w.confidence !== undefined && w.confidence >= 0 && w.confidence <= 1, `"${w.text}" confidence in 0..1`);
    t.true(w.fontName === undefined && w.fontSize === undefined, `"${w.text}" has no font metadata`);
    t.true(w.x1 > w.x0 && w.y1 > w.y0, `"${w.text}" has a positive box`);
  }
};

// ── Tier 2: embedded images mapped through their placement ───────────────────

test("hello-ocr: words come from the embedded image and land inside its placement", (t) => {
  const [p1] = extractLayoutWithOcrPerPage(helloOcr);
  t.is(p1.source, "Ocr");
  t.is(p1.images.length, 1);
  t.is(p1.lines.length, 1);
  t.true(p1.lines[0].text.toLowerCase().includes("hello"), p1.lines[0].text);
  t.true(p1.lines[0].text.toLowerCase().includes("ocr"), p1.lines[0].text);
  assertOcrWords(t, p1);
  const box = p1.images[0];
  for (const w of p1.words) {
    t.true(
      w.x0 >= box.x0 - 1 && w.x1 <= box.x1 + 1 && w.y0 >= box.y0 - 1 && w.y1 <= box.y1 + 1,
      `"${w.text}" inside the image placement`
    );
  }
  t.deepEqual(p1.warnings, []);
});

test("multipage-ocr: every page is OCR'd and positioned", (t) => {
  const pages = extractLayoutWithOcrPerPage(multipageOcr);
  t.is(pages.length, 3);
  for (const p of pages) {
    t.is(p.source, "Ocr", `page ${p.page}`);
    t.true(p.lines.length >= 1, `page ${p.page} has a line`);
    assertOcrWords(t, p);
    t.log(`page ${p.page}: ${lineTexts(p).join(" | ")}`);
  }
});

test("ascii85 + DCT scan: a full page of OCR words in reading order", (t) => {
  const [p1] = extractLayoutWithOcrPerPage(ascii85DctScan);
  t.is(p1.source, "Ocr");
  t.true(p1.lines.length >= 10, `${p1.lines.length} lines`);
  t.true(p1.lines[0].text.toLowerCase().includes("scanned"), p1.lines[0].text);
  for (let i = 1; i < p1.lines.length; i++) {
    t.true(p1.lines[i].y1 <= p1.lines[i - 1].y1 + 0.5, `line ${i} below line ${i - 1}`);
  }
  assertOcrWords(t, p1);
});

// ── Tier 1 passthrough ───────────────────────────────────────────────────────

test("native pages keep their glyph layout and match layoutPerPage exactly", (t) => {
  const viaOcr = extractLayoutWithOcrPerPage(layoutSample);
  t.deepEqual(viaOcr, layoutPerPage(layoutSample));
  t.is(viaOcr[0].source, "Native");
  t.is(viaOcr[0].words[0].fontName, "Helvetica-Bold");
});

test("minTextLength above the page's text falls through the cascade", (t) => {
  const [p1] = extractLayoutWithOcrPerPage(layoutSample, { render: "Never", minTextLength: 999999 });
  // The only raster on the page is a 2×2 gray image: nothing to OCR, and rendering is off.
  t.is(p1.source, "Ocr");
  t.is(p1.words.length, 0);
  t.is(p1.images.length, 1, "image placements from the native pass are kept");
});

// ── Async and class parity ───────────────────────────────────────────────────

test("async and PdfDown match the standalone OCR layout", async (t) => {
  const sync = extractLayoutWithOcrPerPage(helloOcr);
  const pdf = new PdfDown(helloOcr);
  t.deepEqual(await extractLayoutWithOcrPerPageAsync(helloOcr), sync);
  t.deepEqual(pdf.layoutWithOcr(), sync);
  t.deepEqual(await pdf.layoutWithOcrAsync(), sync);
});

// ── Tier 3: rendered page (only when PDFium is available) ────────────────────

if (caps.render) {
  test("render Always: OCR of the rasterised page is mapped onto the page box", (t) => {
    const [p1] = extractLayoutWithOcrPerPage(layoutSample, { render: "Always", renderDpi: 150 });
    t.is(p1.source, "Rendered");
    t.true(p1.lines.length >= 5, `${p1.lines.length} lines`);
    const text = lineTexts(p1).join("\n").toLowerCase();
    t.true(text.includes("layout"), text);
    t.true(text.includes("amount"), text);
    assertOcrWords(t, p1);
    for (const w of p1.words) {
      t.true(w.x0 >= 0 && w.x1 <= p1.width && w.y0 >= 0 && w.y1 <= p1.height, `"${w.text}" inside the page`);
    }
    // The rendered title sits near where the native pass put it.
    const title = p1.words.find((w) => w.text.toLowerCase() === "layout");
    t.truthy(title);
    if (title) {
      t.true(Math.abs(title.x0 - 50) < 6, `title x0 ${title.x0}`);
      t.true(Math.abs(title.y1 - 752.9) < 6, `title y1 ${title.y1}`);
    }
  });
}
