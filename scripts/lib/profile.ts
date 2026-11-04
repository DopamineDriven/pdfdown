import { readFileSync } from "fs";
import { relative } from "path";

import { attachments, extractInvoice, layoutPerPage, pdfMetadata } from "../../index";
import type { EInvoiceStandard, InvoiceSource } from "../../index";
import type { CorpusFile } from "./corpus";
import { fingerprintFonts, fingerprintHeader, producerFamily, sizeClass, textClass } from "./fingerprint";
import type { SizeClass, TextClass } from "./fingerprint";

/** Spec §2.4: pages that are not what their producer says, by word and image count alone. */
export type PageKind = "imageOnly" | "stampOverScan" | "noGlyphsNoImages" | "sparseNative";
const PAGE_KINDS = ["imageOnly", "stampOverScan", "noGlyphsNoImages", "sparseNative"] as const;
const TEXT_CLASSES = ["native", "scanned", "mixed"] as const;
const SIZE_CLASSES = ["Letter", "A4", "landscape", "other"] as const;
const PAGE_BUCKETS = ["1", "2", "3", "4", "5", "6-10", "11-20", "21-50", "51+"] as const;
const COVERAGE_LEVELS = ["50", "80", "90", "95"] as const;
/** A page with at most this many words is sparse (a stamp, a cover sheet, a separator). */
const SPARSE_MAX_WORDS = 30;

type PageBucket = (typeof PAGE_BUCKETS)[number];
type CoverageLevel = (typeof COVERAGE_LEVELS)[number];

/** Per-file facts. `id` first: every row traces back to its file. */
export interface ProfileEntry {
  id: string;
  file: string;
  bytes: number;
  pages: number;
  producer: string | null;
  producerFamily: string;
  textClass: TextClass;
  sizeClass: SizeClass;
  pageSizes: Array<{ width: number; height: number; pages: number }>;
  rotatedPages: number;
  acroForm: boolean;
  xfa: boolean;
  formFields: number;
  attachments: number;
  embeddedXml: EInvoiceStandard | null;
  invoiceSource: InvoiceSource;
  fingerprintFonts: string;
  fingerprintHeader: string;
  layoutMs: number;
  layoutWarnings: number;
  pageKinds: Record<PageKind, number>;
}

/** A file one of the extractors refused; it still carries its id. */
export interface ProfileFailure {
  id: string;
  file: string;
  bytes: number;
  error: string;
}

export interface ClusterTable {
  clusters: number;
  singletons: number;
  /** The six largest cluster sizes, descending. */
  largest: Array<number>;
}

export interface ProfileTables {
  headline: {
    pdfs: number;
    workbooks: number;
    parseFailures: number;
    layoutWarnings: number;
    layoutMs: { total: number; mean: number; max: number };
    textClass: Record<TextClass, number>;
    pagesHistogram: Record<PageBucket, number>;
    sizeClass: Record<SizeClass, number>;
    rotatedPages: number;
    rotatedDocuments: number;
    acroFormDocuments: number;
    xfaDocuments: number;
    attachmentDocuments: number;
    embeddedXmlDocuments: number;
    multiSizeDocuments: number;
  };
  producers: Array<{ family: string; documents: number }>;
  clusters: { fonts: ClusterTable; header: ClusterTable; coverage: Record<CoverageLevel, number> };
  pageKinds: Record<PageKind, { pages: number; documents: number }>;
}

export interface Profile {
  generatedAt: string;
  files: number;
  entries: Array<ProfileEntry>;
  failures: Array<ProfileFailure>;
  tables: ProfileTables;
}

const zeroTextClasses = (): Record<TextClass, number> => ({ native: 0, scanned: 0, mixed: 0 });
const zeroSizeClasses = (): Record<SizeClass, number> => ({ Letter: 0, A4: 0, landscape: 0, other: 0 });
const zeroBuckets = (): Record<PageBucket, number> => ({
  "1": 0,
  "2": 0,
  "3": 0,
  "4": 0,
  "5": 0,
  "6-10": 0,
  "11-20": 0,
  "21-50": 0,
  "51+": 0
});
const zeroKinds = (): Record<PageKind, number> => ({
  imageOnly: 0,
  stampOverScan: 0,
  noGlyphsNoImages: 0,
  sparseNative: 0
});

const pageBucket = (pages: number): PageBucket => {
  if (pages <= 1) return "1";
  if (pages === 2) return "2";
  if (pages === 3) return "3";
  if (pages === 4) return "4";
  if (pages === 5) return "5";
  if (pages <= 10) return "6-10";
  if (pages <= 20) return "11-20";
  if (pages <= 50) return "21-50";
  return "51+";
};

const pageKind = (words: number, images: number): PageKind | undefined => {
  if (words === 0) return images > 0 ? "imageOnly" : "noGlyphsNoImages";
  if (words <= SPARSE_MAX_WORDS) return images > 0 ? "stampOverScan" : "sparseNative";
  return undefined;
};

const clusterTable = (sizes: Array<number>): ClusterTable => ({
  clusters: sizes.length,
  singletons: sizes.filter((n) => n === 1).length,
  largest: sizes.slice(0, 6)
});

/** Clusters needed, largest first, to cover `percent` of `total` documents. */
const clustersToCover = (sizes: Array<number>, total: number, percent: number): number => {
  let covered = 0;
  for (const [i, size] of sizes.entries()) {
    covered += size;
    if (covered * 100 >= total * percent) return i + 1;
  }
  return sizes.length;
};

const clusterSizes = (entries: ReadonlyArray<ProfileEntry>, key: (e: ProfileEntry) => string): Array<number> => {
  const counts = new Map<string, number>();
  for (const e of entries) counts.set(key(e), (counts.get(key(e)) ?? 0) + 1);
  return [...counts.values()].sort((a, b) => b - a);
};

function profileFile(file: CorpusFile, root: string): ProfileEntry | ProfileFailure {
  const buf = readFileSync(file.path);
  const base = { id: file.id, file: relative(root, file.path), bytes: buf.length };
  try {
    const meta = pdfMetadata(buf);
    const started = performance.now();
    const layout = layoutPerPage(buf);
    const layoutMs = performance.now() - started;
    const att = attachments(buf);
    const invoice = extractInvoice(buf);
    const sizes = new Map<string, { width: number; height: number; pages: number }>();
    for (const p of layout) {
      const key = `${Math.round(p.width)}x${Math.round(p.height)}`;
      const size = sizes.get(key) ?? { width: Math.round(p.width), height: Math.round(p.height), pages: 0 };
      size.pages += 1;
      sizes.set(key, size);
    }
    const kinds = zeroKinds();
    for (const p of layout) {
      const kind = pageKind(p.words.length, p.images.length);
      if (kind !== undefined) kinds[kind] += 1;
    }
    const fonts = fingerprintFonts(meta, layout);
    const first = layout[0];
    return {
      ...base,
      pages: meta.pageCount,
      producer: meta.producer ?? null,
      producerFamily: producerFamily(meta.producer),
      textClass: textClass(layout),
      sizeClass: first === undefined ? "other" : sizeClass(first.width, first.height),
      pageSizes: [...sizes.values()],
      rotatedPages: layout.filter((p) => p.rotate !== 0).length,
      acroForm: meta.form?.hasAcroForm ?? false,
      xfa: meta.form?.hasXfa ?? false,
      formFields: meta.form?.fieldCount ?? 0,
      attachments: att.length,
      embeddedXml: invoice.embeddedXml?.standard ?? null,
      invoiceSource: invoice.source,
      fingerprintFonts: fonts,
      fingerprintHeader: fingerprintHeader(fonts, layout),
      layoutMs,
      layoutWarnings: layout.reduce((n, p) => n + p.warnings.length, 0),
      pageKinds: kinds
    };
  } catch (error) {
    return { ...base, error: error instanceof Error ? error.message : String(error) };
  }
}

/** Run every public extractor once per PDF and fold the per-file facts into the §2 tables. */
export function profileCorpus(files: ReadonlyArray<CorpusFile>, root: string): Profile {
  const entries: Array<ProfileEntry> = [];
  const failures: Array<ProfileFailure> = [];
  for (const file of files) {
    if (file.kind !== "pdf") continue;
    const result = profileFile(file, root);
    if ("error" in result) failures.push(result);
    else entries.push(result);
  }
  const workbooks = files.filter((f) => f.kind === "xlsx").length;

  const textClassCounts = zeroTextClasses();
  const sizeClassCounts = zeroSizeClasses();
  const pagesHistogram = zeroBuckets();
  const producers = new Map<string, number>();
  const pageKinds: Record<PageKind, { pages: number; documents: number }> = {
    imageOnly: { pages: 0, documents: 0 },
    stampOverScan: { pages: 0, documents: 0 },
    noGlyphsNoImages: { pages: 0, documents: 0 },
    sparseNative: { pages: 0, documents: 0 }
  };
  let layoutMsTotal = 0;
  let layoutMsMax = 0;
  for (const e of entries) {
    textClassCounts[e.textClass] += 1;
    sizeClassCounts[e.sizeClass] += 1;
    pagesHistogram[pageBucket(e.pages)] += 1;
    producers.set(e.producerFamily, (producers.get(e.producerFamily) ?? 0) + 1);
    for (const kind of PAGE_KINDS) {
      pageKinds[kind].pages += e.pageKinds[kind];
      if (e.pageKinds[kind] > 0) pageKinds[kind].documents += 1;
    }
    layoutMsTotal += e.layoutMs;
    layoutMsMax = Math.max(layoutMsMax, e.layoutMs);
  }
  const fontSizes = clusterSizes(entries, (e) => e.fingerprintFonts);
  const headerSizes = clusterSizes(entries, (e) => e.fingerprintHeader);
  const cover = (percent: number) => clustersToCover(headerSizes, entries.length, percent);
  const coverage: Record<CoverageLevel, number> = {
    "50": cover(50),
    "80": cover(80),
    "90": cover(90),
    "95": cover(95)
  };

  return {
    generatedAt: new Date().toISOString(),
    files: files.length,
    entries,
    failures,
    tables: {
      headline: {
        pdfs: entries.length + failures.length,
        workbooks,
        parseFailures: failures.length,
        layoutWarnings: entries.reduce((n, e) => n + e.layoutWarnings, 0),
        layoutMs: {
          total: Math.round(layoutMsTotal),
          mean: entries.length === 0 ? 0 : Math.round((layoutMsTotal / entries.length) * 10) / 10,
          max: Math.round(layoutMsMax)
        },
        textClass: textClassCounts,
        pagesHistogram,
        sizeClass: sizeClassCounts,
        rotatedPages: entries.reduce((n, e) => n + e.rotatedPages, 0),
        rotatedDocuments: entries.filter((e) => e.rotatedPages > 0).length,
        acroFormDocuments: entries.filter((e) => e.acroForm).length,
        xfaDocuments: entries.filter((e) => e.xfa).length,
        attachmentDocuments: entries.filter((e) => e.attachments > 0).length,
        embeddedXmlDocuments: entries.filter((e) => e.embeddedXml !== null).length,
        multiSizeDocuments: entries.filter((e) => e.pageSizes.length > 1).length
      },
      producers: [...producers.entries()]
        .map(([family, documents]) => ({ family, documents }))
        .sort((a, b) => b.documents - a.documents || a.family.localeCompare(b.family)),
      clusters: { fonts: clusterTable(fontSizes), header: clusterTable(headerSizes), coverage },
      pageKinds
    }
  };
}

const table = (rows: ReadonlyArray<ReadonlyArray<string>>): string => {
  const widths = rows[0]?.map((_, i) => Math.max(...rows.map((r) => (r[i] ?? "").length))) ?? [];
  return rows
    .map((r) =>
      r
        .map((cell, i) => cell.padEnd(widths[i] ?? 0))
        .join("  ")
        .trimEnd()
    )
    .join("\n");
};

/** The §2 tables, aggregates only: no id, file name or page text appears here. */
export function printProfile(profile: Profile): void {
  const { headline, producers, clusters, pageKinds } = profile.tables;
  const out: Array<string> = [];
  out.push("Headline");
  out.push(
    table([
      ["files", `${headline.pdfs} PDFs · ${headline.workbooks} workbooks`],
      ["parse failures", `${headline.parseFailures} of ${headline.pdfs}; ${headline.layoutWarnings} layout warnings`],
      [
        "native layout cost",
        `${(headline.layoutMs.total / 1000).toFixed(1)} s total, ${headline.layoutMs.mean} ms mean, ${headline.layoutMs.max} ms worst`
      ],
      ["text class", TEXT_CLASSES.map((k) => `${headline.textClass[k]} ${k}`).join(" · ")],
      ["pages per document", PAGE_BUCKETS.map((k) => `${k}: ${headline.pagesHistogram[k]}`).join(" · ")],
      ["page size (page 1)", SIZE_CLASSES.map((k) => `${k} ${headline.sizeClass[k]}`).join(" · ")],
      ["/Rotate != 0", `${headline.rotatedPages} pages in ${headline.rotatedDocuments} documents`],
      ["AcroForm present", `${headline.acroFormDocuments} documents (${headline.xfaDocuments} XFA)`],
      ["attachments / embedded XML", `${headline.attachmentDocuments} / ${headline.embeddedXmlDocuments} documents`],
      ["documents with > 1 page size", String(headline.multiSizeDocuments)]
    ])
  );
  out.push("", "Producer families");
  out.push(table([["family", "documents"], ...producers.map((p) => [p.family, String(p.documents)])]));
  out.push("", "Template clusters");
  out.push(
    table([
      ["fingerprint", "clusters", "singletons", "largest"],
      ...(["fonts", "header"] as const).map((k) => [
        k,
        String(clusters[k].clusters),
        String(clusters[k].singletons),
        clusters[k].largest.join(" · ")
      ])
    ])
  );
  out.push(
    `header clusters to cover the corpus: ${COVERAGE_LEVELS.map((l) => `${l}% -> ${clusters.coverage[l]}`).join(", ")}`
  );
  out.push("", "Page kinds");
  out.push(
    table([
      ["kind", "pages", "documents"],
      ...PAGE_KINDS.map((k) => [k, String(pageKinds[k].pages), String(pageKinds[k].documents)])
    ])
  );
  console.log(out.join("\n"));
}
