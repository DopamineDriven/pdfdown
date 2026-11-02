// Local-only OCR run over the scanned invoices in ocr/__test__/fixtures/private/.
// The directory is gitignored: nothing derived from these files reaches the
// repository. Only PDFs with (almost) no native text are OCR'd here; the base
// package's private-corpus spec covers the native ones. Results go to
// report-ocr.json next to the PDFs.

import { existsSync, readdirSync, readFileSync, writeFileSync } from "fs";
import { join } from "path";

import test from "ava";

import { extractLayoutWithOcrPerPage, layoutPerPage } from "../index";
import type { PageLayout, TextSource } from "../index";

/** Native characters below which a document counts as scanned. */
const SCANNED_MAX_NATIVE_CHARS = 20;

interface PrivateOcrPage {
  page: number;
  source: TextSource;
  width: number;
  height: number;
  rotate: number;
  contentRotation: number;
  words: number;
  lines: number;
  images: number;
  meanConfidence: number;
  warnings: Array<string>;
  lineText: Array<string>;
}

interface PrivateOcrEntry {
  file: string;
  pages: number;
  seconds: number;
  layout: Array<PrivateOcrPage>;
}

const PRIVATE = join(import.meta.dirname, "fixtures", "private");

const nativeChars = (pages: Array<PageLayout>) =>
  pages.reduce((n, p) => n + p.words.reduce((m, w) => m + w.text.replace(/\s+/g, "").length, 0), 0);

const scanned = existsSync(PRIVATE)
  ? readdirSync(PRIVATE)
      .filter((n) => /\.pdf$/i.test(n))
      .filter((n) => nativeChars(layoutPerPage(readFileSync(join(PRIVATE, n)))) <= SCANNED_MAX_NATIVE_CHARS)
  : [];

(scanned.length > 0 ? test : test.skip)("private corpus: scanned invoices OCR into positioned pages", (t) => {
  const entries: Array<PrivateOcrEntry> = [];
  t.log(`${scanned.length} scanned invoice(s)`);
  for (const name of scanned) {
    const buf = readFileSync(join(PRIVATE, name));
    const t0 = performance.now();
    const pages = extractLayoutWithOcrPerPage(buf, { render: "Auto" });
    const seconds = (performance.now() - t0) / 1000;
    t.true(pages.length >= 1, `${name}: pages`);
    const layout: Array<PrivateOcrPage> = [];
    for (const p of pages) {
      t.true(p.source === "Ocr" || p.source === "Rendered", `${name} p${p.page}: OCR source, got ${p.source}`);
      t.true([0, 90, 180, 270].includes(p.contentRotation), `${name} p${p.page}: contentRotation`);
      if (p.contentRotation === 90 || p.contentRotation === 270) {
        t.true(p.width > p.height, `${name} p${p.page}: a quarter turn makes the frame landscape`);
        t.true(p.warnings.length >= 1, `${name} p${p.page}: the turn is reported`);
      }
      for (const w of p.words) {
        t.true(w.confidence !== undefined && w.confidence >= 0 && w.confidence <= 1, `${name} p${p.page}: confidence`);
        t.true(
          w.x0 >= -1 && w.x1 <= p.width + 1 && w.y0 >= -1 && w.y1 <= p.height + 1,
          `${name} p${p.page}: "${w.text}" on the page`
        );
      }
      let next = 0;
      for (const [i, line] of p.lines.entries()) {
        t.is(line.firstWord, next, `${name} p${p.page} line ${i}: contiguous`);
        if (i > 0) t.true(line.y1 <= p.lines[i - 1].y1 + 0.5, `${name} p${p.page} line ${i}: top-to-bottom`);
        next += line.wordCount;
      }
      t.is(next, p.words.length, `${name} p${p.page}: lines cover every word`);
      const conf = p.words.map((w) => w.confidence ?? 0);
      const meanConfidence = conf.length ? conf.reduce((a, b) => a + b, 0) / conf.length : 0;
      layout.push({
        page: p.page,
        source: p.source,
        width: p.width,
        height: p.height,
        rotate: p.rotate,
        contentRotation: p.contentRotation,
        words: p.words.length,
        lines: p.lines.length,
        images: p.images.length,
        meanConfidence,
        warnings: p.warnings,
        lineText: p.lines.map((l) => l.text)
      });
      t.log(
        `${name} p${String(p.page).padStart(2)}  ${p.source.padEnd(8)}  rot ${String(p.contentRotation).padStart(3)}  words ${String(p.words.length).padStart(4)}  conf ${meanConfidence.toFixed(2)}`
      );
    }
    entries.push({ file: name, pages: pages.length, seconds, layout });
    t.log(`${name}: ${pages.length} pages in ${seconds.toFixed(1)}s`);
  }
  const report = { generatedAt: new Date().toISOString(), files: entries.length, entries };
  writeFileSync(join(PRIVATE, "report-ocr.json"), `${JSON.stringify(report, null, 2)}\n`);
  t.log(`wrote ${join(PRIVATE, "report-ocr.json")}`);
});
