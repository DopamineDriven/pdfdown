// Local-only OCR run over the scanned invoices in ocr/__test__/fixtures/private/.
// The directory is gitignored: nothing derived from these files reaches the
// repository. Only PDFs with (almost) no native text are OCR'd here; the base
// package's private-corpus spec covers the native ones. Results go to
// report-ocr.json next to the PDFs.

import { existsSync, readFileSync, writeFileSync } from "fs";
import { join, relative } from "path";

import test from "ava";

import { discoverCorpus } from "../../scripts/lib/corpus";
import { extractLayoutWithOcrPerPage, layoutPerPage } from "../index";
import type { PageLayout, TextSource, TextWord } from "../index";

type Box = [number, number, number, number];
const within = (w: TextWord, b: Box) => w.x0 >= b[0] - 1 && w.x1 <= b[2] + 1 && w.y0 >= b[1] - 1 && w.y1 <= b[3] + 1;

/**
 * The engine's placement contract: an OCR word lies on the page, or inside the placement of the
 * image it was read from. Words and placements share one frame, the page's turned by
 * `contentRotation`; a scan can be painted larger than its CropBox, which is why the placement
 * is an acceptable home.
 */
const onPageOrImage = (p: PageLayout, w: TextWord) =>
  within(w, [0, 0, p.width, p.height]) || p.images.some((i) => within(w, [i.x0, i.y0, i.x1, i.y1]));

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
  id: string;
  file: string;
  pages: number;
  seconds: number;
  layout: Array<PrivateOcrPage>;
}

const PRIVATE = join(import.meta.dirname, "fixtures", "private");

const nativeChars = (pages: Array<PageLayout>) =>
  pages.reduce((n, p) => n + p.words.reduce((m, w) => m + w.text.replace(/\s+/g, "").length, 0), 0);

// One level of subdirectories is allowed (the corpus may be grouped by id prefix).
const scanned = existsSync(PRIVATE)
  ? discoverCorpus(PRIVATE)
      .filter((f) => f.kind === "pdf")
      .filter((f) => nativeChars(layoutPerPage(readFileSync(f.path))) <= SCANNED_MAX_NATIVE_CHARS)
  : [];

(scanned.length > 0 ? test : test.skip)("private corpus: scanned invoices OCR into positioned pages", (t) => {
  // Local-only and heavy: every scanned page in the corpus goes through Tesseract (~1.3 s/page here).
  t.timeout(20 * 60 * 1000, "OCR over every scanned document in the private corpus");
  const entries: Array<PrivateOcrEntry> = [];
  t.log(`${scanned.length} scanned invoice(s)`);
  for (const file of scanned) {
    const name = relative(PRIVATE, file.path);
    const buf = readFileSync(file.path);
    const t0 = performance.now();
    const pages = extractLayoutWithOcrPerPage(buf, { render: "Auto" });
    const native = layoutPerPage(buf);
    const seconds = (performance.now() - t0) / 1000;
    t.true(pages.length >= 1, `${name}: pages`);
    const layout: Array<PrivateOcrPage> = [];
    for (const p of pages) {
      t.true(p.source === "Ocr" || p.source === "Rendered", `${name} p${p.page}: OCR source, got ${p.source}`);
      t.true([0, 90, 180, 270].includes(p.contentRotation), `${name} p${p.page}: contentRotation`);
      // A quarter turn swaps the frame's width and height relative to the page's native box.
      const nat = native[p.page - 1];
      const turned = p.contentRotation === 90 || p.contentRotation === 270;
      t.is(p.width, turned ? nat.height : nat.width, `${name} p${p.page}: frame width`);
      t.is(p.height, turned ? nat.width : nat.height, `${name} p${p.page}: frame height`);
      if (p.contentRotation !== 0) {
        t.true(p.warnings.length >= 1, `${name} p${p.page}: the turn is reported`);
      }
      // Hybrid pages keep their native words (no confidence) next to the OCR words (confidence in 0..1).
      const ocrWords = p.words.filter((w) => w.confidence !== undefined);
      t.true(
        ocrWords.length > 0 || p.warnings.some((w) => /OCR found no( new)? words/.test(w)),
        `${name} p${p.page}: OCR words or an explicit no-words warning`
      );
      for (const w of ocrWords) {
        t.true(w.confidence !== undefined && w.confidence >= 0 && w.confidence <= 1, `${name} p${p.page}: confidence`);
        t.true(onPageOrImage(p, w), `${name} p${p.page}: "${w.text}" on the page or inside an image placement`);
      }
      let next = 0;
      for (const [i, line] of p.lines.entries()) {
        t.is(line.firstWord, next, `${name} p${p.page} line ${i}: contiguous`);
        if (i > 0) t.true(line.y1 <= p.lines[i - 1].y1 + 0.5, `${name} p${p.page} line ${i}: top-to-bottom`);
        next += line.wordCount;
      }
      t.is(next, p.words.length, `${name} p${p.page}: lines cover every word`);
      const conf = ocrWords.map((w) => w.confidence ?? 0);
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
    entries.push({ id: file.id, file: name, pages: pages.length, seconds, layout });
    t.log(`${name}: ${pages.length} pages in ${seconds.toFixed(1)}s`);
  }
  const report = { generatedAt: new Date().toISOString(), files: entries.length, entries };
  writeFileSync(join(PRIVATE, "report-ocr.json"), `${JSON.stringify(report, null, 2)}\n`);
  t.log(`wrote ${join(PRIVATE, "report-ocr.json")}`);
});
