import { createHash } from "crypto";

import type { PageLayout, PdfMeta } from "../../index";

export type SizeClass = "Letter" | "A4" | "landscape" | "other";
export type TextClass = "native" | "scanned" | "mixed";

/** A page as the fingerprints and the text class see it: line texts, word texts, image count. */
export interface PageShape {
  lines: ReadonlyArray<{ text: string }>;
  words: ReadonlyArray<{ text: string }>;
  images: ReadonlyArray<unknown>;
}

/** Native characters at or below which a document counts as scanned (spec §2.1). */
export const SCANNED_MAX_NATIVE_CHARS = 20;

/** Producer families worth grouping by name (spec §2.2); any other producer groups by its first word. */
const FAMILIES: ReadonlyArray<readonly [RegExp, string]> = [
  [/itext/i, "iText"],
  [/\bqt\b|wkhtmltopdf/i, "Qt"],
  [/crystal/i, "Crystal Reports"],
  [/microsoft/i, "Microsoft"],
  [/\bsap\b|netweaver/i, "SAP"],
  [/foxit/i, "Foxit"],
  [/pdf-?xchange/i, "PDF-XChange"],
  [/adobe|acrobat|distiller|livecycle/i, "Adobe"],
  [/activereports/i, "ActiveReports"],
  [/chrom(e|ium)|skia/i, "Chromium"],
  [/nitro/i, "Nitro"],
  [/power ?pdf/i, "Power PDF"],
  [/reportbuilder/i, "ReportBuilder"],
  [/pdfium/i, "PDFium"],
  [/devexpress/i, "DevExpress"],
  [/clibpdf/i, "ClibPDF"]
];

/** The family a `/Producer` string belongs to: a known generator, else its first word, else "unknown". */
export function producerFamily(producer: string | undefined): string {
  const text = (producer ?? "").trim();
  if (text === "") return "unknown";
  const known = FAMILIES.find(([pattern]) => pattern.test(text));
  if (known !== undefined) return known[1];
  return text.split(/[\s:(,;/]+/)[0] || "unknown";
}

/** Page-1 size class: Letter and A4 within two points, anything wider than tall is landscape. */
export function sizeClass(width: number, height: number): SizeClass {
  if (width > height + 1) return "landscape";
  if (Math.abs(width - 612) <= 2 && Math.abs(height - 792) <= 2) return "Letter";
  if (Math.abs(width - 595.3) <= 2 && Math.abs(height - 841.9) <= 2) return "A4";
  return "other";
}

/** Producer family + page-1 size class + the sorted font names of page 1. */
export function fingerprintFonts(meta: PdfMeta, layout: ReadonlyArray<PageLayout>): string {
  const first = layout[0];
  const size = first === undefined ? "none" : sizeClass(first.width, first.height);
  const fonts =
    first === undefined
      ? []
      : [...new Set(first.words.map((w) => w.fontName).filter((n): n is string => n !== undefined))].sort();
  return `${producerFamily(meta.producer)}|${size}|${fonts.join(",")}`;
}

/**
 * The fonts fingerprint plus a hash of page 1's first four line texts with every digit masked, so
 * two documents from one template share it whatever their numbers say.
 */
export function fingerprintHeader(fonts: string, layout: ReadonlyArray<Pick<PageShape, "lines">>): string {
  const header = (layout[0]?.lines ?? [])
    .slice(0, 4)
    .map((l) => l.text.replace(/\d/g, "#").replace(/\s+/g, " ").trim())
    .join("\n");
  return `${fonts}|${createHash("sha1").update(header).digest("hex").slice(0, 12)}`;
}

export const pageNativeChars = (page: Pick<PageShape, "words">): number =>
  page.words.reduce((n, w) => n + w.text.replace(/\s+/g, "").length, 0);

/** A page with (almost) no glyphs and at least one image: a scan, inside a document that has native pages too. */
export const isImageOnlyPage = (page: Pick<PageShape, "words" | "images">): boolean =>
  pageNativeChars(page) <= SCANNED_MAX_NATIVE_CHARS && page.images.length > 0;

/**
 * `scanned` when the whole document has at most SCANNED_MAX_NATIVE_CHARS native characters (the
 * spec's rule, whatever paints the pages), `mixed` when native pages sit next to image-only ones,
 * `native` otherwise; a blank separator page is not image-only.
 */
export function textClass(layout: ReadonlyArray<Pick<PageShape, "words" | "images">>): TextClass {
  const chars = layout.reduce((n, p) => n + pageNativeChars(p), 0);
  if (chars <= SCANNED_MAX_NATIVE_CHARS) return "scanned";
  return layout.some(isImageOnlyPage) ? "mixed" : "native";
}
