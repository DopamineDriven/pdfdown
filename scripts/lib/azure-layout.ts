// Azure Document Intelligence read results as pdfdown pages: the words, lines and page boxes of a
// `prebuilt-read` / `prebuilt-layout` analysis mapped onto `PageLayout`, so `invoiceFromLayout`
// runs the same reader over another engine's OCR. Azure measures in inches (or pixels for images)
// from the top-left corner with eight-number polygons and groups words into text regions;
// pdfdown pages are points from the bottom-left corner with boxes and group words by baseline.
// Nothing here knows about invoices.
import type { PageLayout, TextLine, TextWord } from "../../index";

/** The part of an Azure Document Intelligence analyze result this adapter reads. */
export interface AzureReadResult {
  pages: Array<AzurePage>;
}

export interface AzurePage {
  pageNumber: number;
  angle?: number;
  width: number;
  height: number;
  /** `"inch"` for PDFs, `"pixel"` for images. */
  unit: string;
  words: Array<AzureWord>;
  lines: Array<AzureLine>;
}

export interface AzureSpan {
  offset: number;
  length: number;
}

export interface AzureWord {
  content: string;
  /** Eight numbers: four corners, clockwise from the top-left, in the page's unit. */
  polygon: Array<number>;
  confidence: number;
  span: AzureSpan;
}

export interface AzureLine {
  content: string;
  polygon: Array<number>;
  spans: Array<AzureSpan>;
}

const POINTS_PER_INCH = 72;
/** Pixel pages carry no physical size; 96 pixels per inch is the common screen assumption. */
const POINTS_PER_PIXEL = 72 / 96;
/** A word box spans ascenders to descenders, about 1.2 em of its font size. */
const BOX_HEIGHT_PER_EM = 1.2;

interface Box {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

/** The polygon's bounding box in points, y up from the page's bottom edge. */
function box(polygon: Array<number>, pageHeight: number, scale: number): Box {
  const xs = polygon.filter((_, i) => i % 2 === 0);
  const ys = polygon.filter((_, i) => i % 2 === 1);
  const left = Math.min(...xs);
  const right = Math.max(...xs);
  const top = Math.min(...ys);
  const bottom = Math.max(...ys);
  return {
    x0: left * scale,
    x1: right * scale,
    y0: (pageHeight - bottom) * scale,
    y1: (pageHeight - top) * scale
  };
}

function scaleOf(unit: string): number {
  return unit === "pixel" ? POINTS_PER_PIXEL : POINTS_PER_INCH;
}

/** Two boxes share a baseline when their vertical centres sit within half the taller box. */
function sameBaseline(a: Box, b: Box): boolean {
  const ca = (a.y0 + a.y1) / 2;
  const cb = (b.y0 + b.y1) / 2;
  const tall = Math.max(a.y1 - a.y0, b.y1 - b.y0);
  return Math.abs(ca - cb) <= 0.5 * tall;
}

/**
 * One Azure page as a pdfdown page. Azure's lines are text regions (a label and its value in
 * the next column are two lines), while pdfdown's lines are baselines, so the words are regrouped
 * by baseline, top of the page first and left to right within a line, the way the native layout
 * builder groups them.
 */
export function azurePageToLayout(page: AzurePage): PageLayout {
  const scale = scaleOf(page.unit);
  const warnings: Array<string> = [];
  if (page.unit !== "inch" && page.unit !== "pixel") {
    warnings.push(`unit "${page.unit}" is not inch or pixel; coordinates were taken as points`);
  }
  const boxed = page.words
    .map((w) => ({ word: w, box: box(w.polygon, page.height, scale) }))
    .sort((a, b) => (b.box.y0 + b.box.y1) / 2 - (a.box.y0 + a.box.y1) / 2);
  const groups: Array<{ bounds: Box; members: Array<{ word: AzureWord; box: Box }> }> = [];
  for (const item of boxed) {
    const last = groups[groups.length - 1];
    if (last !== undefined && sameBaseline(last.bounds, item.box)) {
      last.members.push(item);
      last.bounds = {
        x0: Math.min(last.bounds.x0, item.box.x0),
        y0: Math.min(last.bounds.y0, item.box.y0),
        x1: Math.max(last.bounds.x1, item.box.x1),
        y1: Math.max(last.bounds.y1, item.box.y1)
      };
    } else {
      groups.push({ bounds: { ...item.box }, members: [item] });
    }
  }
  const words: Array<TextWord> = [];
  const lines: Array<TextLine> = [];
  for (const [lineIndex, group] of groups.entries()) {
    group.members.sort((a, b) => a.box.x0 - b.box.x0);
    const firstWord = words.length;
    for (const { word: w, box: b } of group.members) {
      words.push({
        text: w.content,
        ...b,
        line: lineIndex,
        fontSize: (b.y1 - b.y0) / BOX_HEIGHT_PER_EM,
        confidence: w.confidence
      });
    }
    lines.push({
      text: group.members.map((m) => m.word.content).join(" "),
      ...group.bounds,
      firstWord,
      wordCount: group.members.length
    });
  }
  if (page.angle !== undefined && Math.abs(page.angle) > 0.5) {
    warnings.push(`Azure reports the page turned ${page.angle.toFixed(1)}°; boxes are its upright read`);
  }
  return {
    page: page.pageNumber,
    width: page.width * scale,
    height: page.height * scale,
    rotate: 0,
    contentRotation: 0,
    source: "Ocr",
    words,
    lines,
    images: [],
    warnings
  };
}

/** Every page of an Azure read result as pdfdown pages, in page order. */
export function azureToLayout(result: AzureReadResult): Array<PageLayout> {
  return [...result.pages].sort((a, b) => a.pageNumber - b.pageNumber).map(azurePageToLayout);
}
