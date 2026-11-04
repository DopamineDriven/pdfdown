import { copyFileSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";

import test from "ava";

import { extractInvoice } from "../../index";
import { discoverCorpus } from "../lib/corpus";
import { evaluateCorpus, renderEval } from "../lib/evaluate";
import type { EvalReport } from "../lib/evaluate";
import { parseExpected, readExpected, toExpected } from "../lib/expected";
import type { ExpectedInvoice } from "../lib/expected";
import { HEADER_FIELDS, aggregate, reconciles, scoreDocument } from "../lib/metrics";
import type { DocScore } from "../lib/metrics";

const FIXTURES = join(import.meta.dirname, "..", "..", "__test__", "fixtures");
const facturx = readFileSync(join(FIXTURES, "einvoice", "facturx-minimal.pdf"));
const facturxExpected = join(FIXTURES, "einvoice", "facturx-minimal.expected.json");

const asDoc = (id: string, score: ReturnType<typeof scoreDocument>, cluster: string | null = null): DocScore => ({
  id,
  source: "EmbeddedXml",
  cluster,
  textClass: "native",
  reconciled: null,
  ...score
});

test("a deliberately wrong expected file scores 0 on every header field and on items, and does not throw", (t) => {
  const actual = toExpected(extractInvoice(facturx));
  const wrong: ExpectedInvoice = {
    invoiceId: "NOPE-1",
    invoiceDate: "1999-01-01",
    invoiceTotal: 1,
    amountDue: 2,
    purchaseOrder: "PO-0",
    documentType: "CreditNote",
    vendorName: "Someone Else",
    customerName: "Nobody",
    shippingAddress: { lines: ["Nowhere 1"] },
    items: [{ productCode: "X", amount: 1 }]
  };
  const score = scoreDocument(actual, wrong);
  for (const field of HEADER_FIELDS) t.is(score.fields[field], 0, field);
  t.is(score.items.recall, 0);
  t.is(score.items.precision, 0);
  t.is(score.weight, 1, "the expected total is the document's weight even when the extraction disagrees");
  t.true(score.hasAmount);
});

test("the FacturX fixture matches its expected file: header fields, items and totals reconcile", (t) => {
  const result = extractInvoice(facturx);
  const score = scoreDocument(toExpected(result), readExpected(facturxExpected));
  t.is(score.fields.invoiceId, 1);
  t.is(score.fields.invoiceDate, 1);
  t.is(score.fields.invoiceTotal, 1);
  t.is(score.fields.amountDue, 1);
  t.is(score.fields.purchaseOrder, 1);
  t.is(score.fields.documentType, 1);
  t.is(score.fields.vendorName, 1);
  t.is(score.fields.customerName, 1);
  t.is(score.fields.shippingAddress, null, "a field the expected file does not state is not scored");
  t.deepEqual(score.items, { expected: 2, actual: 2, matched: 2, precision: 1, recall: 1 });
  t.is(score.weight, 833);
  t.is(reconciles(result), true, "500 + 200 is the net total; tax sits between it and the 833 due");
});

test("comparison is forgiving about case, whitespace and cents, strict about values", (t) => {
  const actual: ExpectedInvoice = {
    invoiceId: " inv-2026-0042 ",
    invoiceDate: "2026-11-01",
    invoiceTotal: 833.004,
    vendorName: "ACME  Aviation Parts GmbH"
  };
  const expected: ExpectedInvoice = {
    invoiceId: "INV-2026-0042",
    invoiceDate: "2026-11-02",
    invoiceTotal: 833,
    vendorName: "Acme Aviation Parts GmbH",
    customerName: "Missing In Action"
  };
  const score = scoreDocument(actual, expected);
  t.is(score.fields.invoiceId, 1);
  t.is(score.fields.invoiceTotal, 1);
  t.is(score.fields.vendorName, 1);
  t.is(score.fields.invoiceDate, 0, "a different day is a miss");
  t.is(score.fields.customerName, 0, "expected but not extracted is a miss");
  t.is(score.fields.purchaseOrder, null, "not expected, not scored");
});

test("an expected file without invoiceTotal counts one document and no amount, and the table says so", (t) => {
  const result = extractInvoice(facturx);
  const full = asDoc("100001", scoreDocument(toExpected(result), readExpected(facturxExpected)), "c1");
  const { invoiceTotal: _total, ...withoutTotal } = readExpected(facturxExpected);
  const noAmount = asDoc("100002", scoreDocument(toExpected(result), { ...withoutTotal, invoiceId: "WRONG" }), "c1");
  const table = aggregate([full, noAmount]);
  t.is(table.all.documents, 2);
  t.is(table.all.missingAmounts, 1);
  t.is(table.all.amountTotal, 833);
  t.is(table.all.fields.invoiceId.scored, 2);
  t.is(table.all.fields.invoiceId.documentWeighted, 0.5, "one of two documents matched");
  t.is(table.all.fields.invoiceId.amountWeighted, 1, "the only document with an amount matched");
  t.is(table.all.fields.invoiceTotal.scored, 1);
  t.is(table.all.fields.shippingAddress.documentWeighted, null, "never expected, never scored");
  t.is(table.clusters.length, 1);
  t.is(table.clusters[0].documents, 2);
  t.is(table.all.items.recall, 1);
});

test("parseExpected keeps the known keys with the right types and reports the rest", (t) => {
  const { expected, warnings } = parseExpected({
    invoiceId: 12345,
    invoiceDate: "2026-11-01",
    invoiceTotal: "833",
    vendorName: "Acme",
    items: [{ productCode: "A", amount: 1 }, "junk"],
    somethingElse: true
  });
  t.deepEqual(expected, { invoiceDate: "2026-11-01", vendorName: "Acme", items: [{ productCode: "A", amount: 1 }] });
  t.is(warnings.length, 4, warnings.join(" | "));
});

test("baseline: sample invoices without a heuristic track score 0% with source None, ids without an expected file are skipped and counted", (t) => {
  const root = mkdtempSync(join(tmpdir(), "pdfdown-eval-"));
  try {
    copyFileSync(join(FIXTURES, "invoices", "wordpress-pdf-invoice-plugin-sample.pdf"), join(root, "100001.pdf"));
    copyFileSync(join(FIXTURES, "invoices", "Downloadable-PDF-Invoices-Add-On-Samples.pdf"), join(root, "100002.pdf"));
    copyFileSync(join(FIXTURES, "einvoice", "facturx-minimal.pdf"), join(root, "100003.pdf"));
    writeFileSync(join(root, "100001.expected.json"), JSON.stringify({ invoiceId: "INV-3337", invoiceTotal: 93.5 }));
    writeFileSync(join(root, "100002.expected.json"), JSON.stringify({ invoiceId: "1", invoiceDate: "2013-01-01" }));
    const report = evaluateCorpus(discoverCorpus(root), root, {});
    t.is(report.documents, 3);
    t.is(report.scored, 2);
    t.is(report.skipped, 1, "the PDF without an expected file");
    t.deepEqual(report.failures, []);
    t.deepEqual(
      report.scores.map((s) => [s.id, s.source]),
      [
        ["100001", "None"],
        ["100002", "None"]
      ]
    );
    t.is(report.table.all.fields.invoiceId.documentWeighted, 0);
    t.is(report.table.all.fields.invoiceTotal.amountWeighted, 0);
    t.is(report.table.all.missingAmounts, 1);
    t.is(report.table.slice, null, "no --ids slice requested");
    const sliced = evaluateCorpus(discoverCorpus(root), root, { ids: new Set(["100002"]) });
    t.is(sliced.table.slice?.documents, 1);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

// ── Review fixes ─────────────────────────────────────────────────────────────

const reportOf = (scores: Array<DocScore>, ids?: ReadonlySet<string>): EvalReport => ({
  generatedAt: "2026-11-04T00:00:00.000Z",
  documents: scores.length,
  scored: scores.length,
  skipped: 0,
  failures: [],
  expectedWarnings: [],
  scores,
  table: aggregate(scores, { ids })
});

test("default output never prints a single document's amount: totals appear only for rows of three or more documents", (t) => {
  const result = extractInvoice(facturx);
  const expected = readExpected(facturxExpected);
  const score = scoreDocument(toExpected(result), expected);
  const scores = [
    asDoc("100001", score, "big"),
    asDoc("100002", score, "big"),
    asDoc("100003", score, "big"),
    asDoc("100004", score, "lonely")
  ];
  const lines = renderEval(reportOf(scores, new Set(["100004"])));
  const header = (label: string) => lines.find((l) => l.startsWith(`${label}:`)) ?? "";
  t.true(header("all").includes("amount total"), header("all"));
  t.true(header("big").includes("amount total"), header("big"));
  t.false(header("lonely").includes("amount total"), header("lonely"));
  t.false(header("ids slice (1 listed)").includes("amount total"), header("ids slice (1 listed)"));
  t.false(
    lines.some((l) => l.includes("833") && !l.includes("amount total")),
    "no other line carries an amount"
  );
});

test("aircraft serial numbers are scored like items", (t) => {
  const actual: ExpectedInvoice = { aircraft: [{ serialNumber: "20123" }, { serialNumber: " 9000 " }] };
  t.deepEqual(scoreDocument(actual, { aircraft: [{ serialNumber: "20123" }] }).aircraft, {
    expected: 1,
    actual: 2,
    matched: 1,
    precision: 0.5,
    recall: 1
  });
  t.deepEqual(scoreDocument(actual, { aircraft: [{ serialNumber: "9000" }, { serialNumber: "1" }] }).aircraft, {
    expected: 2,
    actual: 2,
    matched: 1,
    precision: 0.5,
    recall: 0.5
  });
  t.is(scoreDocument(actual, {}).aircraft.recall, null, "not stated, not scored");
  const row = aggregate([asDoc("100001", scoreDocument(actual, { aircraft: [{ serialNumber: "20123" }] }))]).all;
  t.deepEqual(row.aircraft, { documents: 1, precision: 0.5, recall: 1 });
});

test("a credit note's negative total weighs by its magnitude", (t) => {
  const invoice = asDoc(
    "100001",
    scoreDocument({ invoiceId: "A", invoiceTotal: 1000 }, { invoiceId: "A", invoiceTotal: 1000 })
  );
  const credit = asDoc(
    "100002",
    scoreDocument({ invoiceId: "X", invoiceTotal: -900 }, { invoiceId: "C", invoiceTotal: -900 })
  );
  const row = aggregate([invoice, credit]).all;
  t.is(row.amountTotal, 1900);
  t.is(row.fields.invoiceId.documentWeighted, 0.5);
  t.is(row.fields.invoiceId.amountWeighted, 1000 / 1900);
});

test("items match on product code when both sides have one, else on description; differing codes do not match", (t) => {
  const items = (actual: ExpectedInvoice["items"], expected: ExpectedInvoice["items"]) =>
    scoreDocument({ items: actual }, { items: expected }).items.matched;
  t.is(
    items(
      [{ productCode: "HPS-220", description: "Seal kit", amount: 500 }],
      [{ description: "Seal kit", amount: 500 }]
    ),
    1
  );
  t.is(
    items(
      [{ description: "Seal kit", amount: 500 }],
      [{ productCode: "HPS-220", description: "Seal kit", amount: 500 }]
    ),
    1
  );
  t.is(
    items(
      [{ productCode: "A", description: "Seal kit", amount: 500 }],
      [{ productCode: "B", description: "Seal kit", amount: 500 }]
    ),
    0
  );
  t.is(
    items(
      [{ productCode: "A", description: "Seal kit", amount: 500 }],
      [{ productCode: "A", description: "Other", amount: 500 }]
    ),
    1
  );
  t.is(items([{ description: "Seal kit", amount: 500 }], [{ description: "Seal kit", amount: 501 }]), 0);
});
