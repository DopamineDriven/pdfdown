import { readFileSync } from "fs";
import { join } from "path";

import test from "ava";

import { layoutPerPage, pdfMetadata } from "../../index";
import { discoverCorpus } from "../lib/corpus";
import { fingerprintFonts, fingerprintHeader, producerFamily, sizeClass, textClass } from "../lib/fingerprint";
import { profileCorpus } from "../lib/profile";

const FIXTURES = join(import.meta.dirname, "..", "..", "__test__", "fixtures");
const load = (...parts: Array<string>) => readFileSync(join(FIXTURES, ...parts));

/** A page carrying only line texts and words, which is all the fingerprints and text class read. */
const page = (lines: Array<string>, images = 0) => ({
  lines: lines.map((text) => ({ text })),
  words: lines.flatMap((l) => l.split(/\s+/).filter(Boolean)).map((text) => ({ text })),
  images: Array.from({ length: images }, () => ({}))
});

test("producer family: known generators by name, the long tail by first word, blank is unknown", (t) => {
  t.is(producerFamily("iText 2.1.7 by 1T3XT"), "iText");
  t.is(producerFamily("Qt 4.8.7"), "Qt");
  t.is(producerFamily("wkhtmltopdf"), "Qt");
  t.is(producerFamily("Crystal Reports"), "Crystal Reports");
  t.is(producerFamily("Microsoft: Print To PDF"), "Microsoft");
  t.is(producerFamily("Microsoft® Word for Microsoft 365"), "Microsoft");
  t.is(producerFamily("SAP NetWeaver 750"), "SAP");
  t.is(producerFamily("Foxit Reader PDF Printer Version 9.7.0.29435"), "Foxit");
  t.is(producerFamily("PDF-XChange Standard (7.0 build 328.2)"), "PDF-XChange");
  t.is(producerFamily("Adobe PDF Library 15.0"), "Adobe");
  t.is(producerFamily("Acrobat Distiller 11.0 (Windows)"), "Adobe");
  t.is(producerFamily("Adobe LiveCycle Designer ES 9.0: XML Form Module"), "Adobe");
  t.is(producerFamily("TCPDF 6.0.051 (http://www.tcpdf.org)"), "TCPDF");
  t.is(producerFamily("ReportLab PDF Library - (opensource)"), "ReportLab");
  t.is(producerFamily(undefined), "unknown");
  t.is(producerFamily("   "), "unknown");
});

test("size class: Letter, A4, landscape, other", (t) => {
  t.is(sizeClass(612, 792), "Letter");
  t.is(sizeClass(595.2, 841.68), "A4");
  t.is(sizeClass(792, 612), "landscape");
  t.is(sizeClass(500, 700), "other");
});

test("header fingerprint: stable across two runs, blind to digits, sensitive to letters", (t) => {
  const facturx = load("einvoice", "facturx-minimal.pdf");
  const once = fingerprintHeader(
    fingerprintFonts(pdfMetadata(facturx), layoutPerPage(facturx)),
    layoutPerPage(facturx)
  );
  const twice = fingerprintHeader(
    fingerprintFonts(pdfMetadata(facturx), layoutPerPage(facturx)),
    layoutPerPage(facturx)
  );
  t.is(once, twice);
  const sample = load("invoices", "wordpress-pdf-invoice-plugin-sample.pdf");
  const sampleFonts = fingerprintFonts(pdfMetadata(sample), layoutPerPage(sample));
  t.is(fingerprintHeader(sampleFonts, layoutPerPage(sample)), fingerprintHeader(sampleFonts, layoutPerPage(sample)));
  t.true(sampleFonts.includes("Helvetica"), sampleFonts);

  const fonts = "unknown|Letter|Helvetica";
  const base = fingerprintHeader(fonts, [page(["Invoice 12345", "Date 2026-11-04", "PO 4711", "Total 99.00"])]);
  const digits = fingerprintHeader(fonts, [page(["Invoice 67890", "Date 2025-01-31", "PO 1234", "Total 10.50"])]);
  const letters = fingerprintHeader(fonts, [page(["Credit 12345", "Date 2026-11-04", "PO 4711", "Total 99.00"])]);
  const fifthLine = fingerprintHeader(fonts, [
    page(["Invoice 12345", "Date 2026-11-04", "PO 4711", "Total 99.00", "Extra"])
  ]);
  t.is(digits, base, "digits are masked");
  t.not(letters, base, "a letter change is a different template");
  t.is(fifthLine, base, "only the first four lines count");
  t.true(base.startsWith(`${fonts}|`), base);
});

test("text class: a scan is scanned, a sparse native page is still native, a native plus an image-only page is mixed", (t) => {
  t.is(textClass(layoutPerPage(load("ascii85-dct-scan.pdf"))), "scanned");
  t.is(textClass(layoutPerPage(load("layout", "layout-sample.pdf"))), "native");
  t.is(textClass([page(["A native page with plenty of words on it to count"]), page([], 1)]), "mixed");
  t.is(
    textClass([page(["A native page with plenty of words on it to count"]), page([])]),
    "native",
    "a blank separator is not an image-only page"
  );
});

test("profile of the public invoice fixtures: deterministic counts, every entry traceable by id", (t) => {
  const files = [...discoverCorpus(join(FIXTURES, "invoices")), ...discoverCorpus(join(FIXTURES, "einvoice"))].filter(
    (f) => f.kind === "pdf"
  );
  const profile = profileCorpus(files, FIXTURES);
  t.deepEqual(
    profile.entries.map((e) => e.id),
    [
      "Downloadable-PDF-Invoices-Add-On-Samples",
      "wordpress-pdf-invoice-plugin-sample",
      "einvoice-broken",
      "facturx-minimal",
      "ubl-attachments"
    ]
  );
  const { headline, producers, clusters, pageKinds } = profile.tables;
  t.is(headline.pdfs, 5);
  t.is(headline.parseFailures, 0);
  t.deepEqual(headline.textClass, { native: 2, scanned: 3, mixed: 0 });
  t.deepEqual(headline.pagesHistogram, {
    "1": 3,
    "2": 1,
    "3": 0,
    "4": 1,
    "5": 0,
    "6-10": 0,
    "11-20": 0,
    "21-50": 0,
    "51+": 0
  });
  t.deepEqual(headline.sizeClass, { Letter: 3, A4: 2, landscape: 0, other: 0 });
  t.is(headline.rotatedPages, 0);
  t.is(headline.rotatedDocuments, 0);
  t.is(headline.acroFormDocuments, 1, "a report generator left an empty AcroForm on one sample");
  t.is(headline.attachmentDocuments, 3);
  t.is(headline.embeddedXmlDocuments, 2);
  t.is(headline.multiSizeDocuments, 0);
  t.true(headline.layoutMs.total > 0 && headline.layoutMs.max >= headline.layoutMs.mean);
  t.deepEqual(producers, [
    { family: "unknown", documents: 4 },
    { family: "TCPDF", documents: 1 }
  ]);
  t.deepEqual(clusters.fonts, { clusters: 3, singletons: 2, largest: [3, 1, 1] });
  t.deepEqual(clusters.header, { clusters: 3, singletons: 2, largest: [3, 1, 1] });
  t.deepEqual(clusters.coverage, { "50": 1, "80": 2, "90": 3, "95": 3 });
  t.deepEqual(pageKinds, {
    imageOnly: { pages: 0, documents: 0 },
    stampOverScan: { pages: 0, documents: 0 },
    noGlyphsNoImages: { pages: 4, documents: 3 },
    sparseNative: { pages: 0, documents: 0 }
  });
  t.true(profile.entries.every((e) => e.fingerprintHeader.startsWith(`${e.fingerprintFonts}|`)));
  t.true(
    profile.entries.every((e) => e.file !== undefined && !e.file.startsWith("/")),
    "file is relative to the root"
  );
});

test("profile of the public workbook fixtures: every .xlsx is a row led by its id; a Strict OOXML workbook is a failure, not a crash", (t) => {
  const files = discoverCorpus(join(FIXTURES, "spreadsheet"));
  t.deepEqual(
    files.map((f) => f.id),
    ["date1904", "form", "minimal", "strict"],
    ".xlsm, .docx and .bin are not corpus files"
  );
  const profile = profileCorpus(files, FIXTURES);
  const { headline } = profile.tables;
  t.is(headline.pdfs, 0);
  t.is(headline.workbooks, 4);
  t.is(headline.workbooksParsed, 3);
  t.is(headline.parseFailures, 0, "PDF failures only");
  t.deepEqual(
    profile.workbooks.map((w) => w.id),
    ["date1904", "form", "minimal"]
  );
  t.deepEqual(
    profile.failures.map((f) => [f.id, f.kind]),
    [["strict", "xlsx"]]
  );
  t.regex(profile.failures[0]?.error ?? "", /Strict OOXML/);
  const form = profile.workbooks.find((w) => w.id === "form");
  t.like(form, {
    file: "spreadsheet/form.xlsx",
    sheets: 2,
    hiddenSheets: 1,
    cells: 24,
    merges: 2,
    formulas: 2,
    unsupportedFormats: 0,
    pages: 1,
    cellTypes: { Text: 13, Number: 7, Boolean: 1, Error: 1, Date: 1, Blank: 1 }
  });
  t.is(Object.keys(form?.formats ?? {}).length, 6, "six distinct codes on the numeric cells");
  t.is(form?.formats["m/d/yyyy"], 2, "the date column and the serial-60 cell share built-in 14");
  t.true((form?.layoutMs ?? -1) >= 0);
  t.true(
    profile.workbooks.every((w) => !w.file.startsWith("/")),
    "file is relative to the root"
  );
});
