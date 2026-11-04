import { existsSync, readFileSync } from "fs";
import { relative } from "path";

import { extractInvoice } from "../../index";
import type { CorpusFile } from "./corpus";
import { expectedPathFor, parseExpected, toExpected } from "./expected";
import type { TextClass } from "./fingerprint";
import { HEADER_FIELDS, aggregate, reconciles, scoreDocument } from "./metrics";
import type { DocScore, Row, Table } from "./metrics";

/** What the harness takes from `profile.json` for a document, when the profile exists. */
export interface ProfileLookup {
  cluster: string | null;
  textClass: TextClass | null;
}

export interface EvalFailure {
  id: string;
  file: string;
  error: string;
}

export interface EvalReport {
  generatedAt: string;
  /** PDFs in the corpus. */
  documents: number;
  /** PDFs with an expected file that were scored. */
  scored: number;
  /** PDFs without an `<id>.expected.json`. */
  skipped: number;
  failures: Array<EvalFailure>;
  /** Expected files whose contents were partly dropped, by id. */
  expectedWarnings: Array<{ id: string; warnings: Array<string> }>;
  scores: Array<DocScore>;
  table: Table;
}

export interface EvaluateOptions {
  /** Ids to report as a slice of their own (`--ids`); the harness never knows why they were chosen. */
  ids?: ReadonlySet<string>;
  lookup?: ReadonlyMap<string, ProfileLookup>;
}

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const isTextClass = (v: unknown): v is TextClass => v === "native" || v === "scanned" || v === "mixed";

/** Cluster and text class per id from a parsed `profile.json`; entries it cannot read are skipped. */
export function profileLookup(profileJson: unknown): Map<string, ProfileLookup> {
  const lookup = new Map<string, ProfileLookup>();
  if (!isRecord(profileJson) || !Array.isArray(profileJson.entries)) return lookup;
  for (const entry of profileJson.entries) {
    if (!isRecord(entry) || typeof entry.id !== "string") continue;
    lookup.set(entry.id, {
      cluster: typeof entry.fingerprintHeader === "string" ? entry.fingerprintHeader : null,
      textClass: isTextClass(entry.textClass) ? entry.textClass : null
    });
  }
  return lookup;
}

/** Score every PDF that has an expected file beside it; skip and count the rest. */
export function evaluateCorpus(files: ReadonlyArray<CorpusFile>, root: string, options: EvaluateOptions): EvalReport {
  const pdfs = files.filter((f) => f.kind === "pdf");
  const scores: Array<DocScore> = [];
  const failures: Array<EvalFailure> = [];
  const expectedWarnings: Array<{ id: string; warnings: Array<string> }> = [];
  let skipped = 0;
  for (const file of pdfs) {
    const expectedPath = expectedPathFor(file.path);
    if (!existsSync(expectedPath)) {
      skipped += 1;
      continue;
    }
    try {
      const { expected, warnings } = parseExpected(JSON.parse(readFileSync(expectedPath, "utf8")));
      if (warnings.length > 0) expectedWarnings.push({ id: file.id, warnings });
      const result = extractInvoice(readFileSync(file.path));
      const meta = options.lookup?.get(file.id);
      scores.push({
        id: file.id,
        source: result.source,
        cluster: meta?.cluster ?? null,
        textClass: meta?.textClass ?? null,
        reconciled: reconciles(result),
        ...scoreDocument(toExpected(result), expected)
      });
    } catch (error) {
      failures.push({
        id: file.id,
        file: relative(root, file.path),
        error: error instanceof Error ? error.message : String(error)
      });
    }
  }
  return {
    generatedAt: new Date().toISOString(),
    documents: pdfs.length,
    scored: scores.length,
    skipped,
    failures,
    expectedWarnings,
    scores,
    table: aggregate(scores, { ids: options.ids })
  };
}

const pct = (x: number | null): string => (x === null ? "-" : `${(x * 100).toFixed(1)}%`);
const money = (x: number): string => x.toLocaleString("en-US", { maximumFractionDigits: 2 });

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

/** One row as a printed block: the header line, then a field table with both weightings. */
function printRow(r: Row, out: Array<string>): void {
  out.push(
    `${r.label}: ${r.documents} documents, ${r.missingAmounts} without an expected total, amount total ${money(r.amountTotal)}`
  );
  out.push(
    table([
      ["field", "scored", "document-weighted", "amount-weighted"],
      ...HEADER_FIELDS.map((f) => [
        f,
        String(r.fields[f].scored),
        pct(r.fields[f].documentWeighted),
        pct(r.fields[f].amountWeighted)
      ]),
      ["items (P / R)", String(r.items.documents), pct(r.items.precision), pct(r.items.recall)],
      ["totals reconcile", String(r.reconciled.documents), pct(r.reconciled.rate), ""]
    ])
  );
  out.push("");
}

/** Aggregates only: clusters are named by their fingerprint, never by a vendor or file. */
export function printEval(report: EvalReport, maxClusters = 10): void {
  const out: Array<string> = [];
  out.push(
    `${report.documents} PDFs; ${report.scored} scored against an expected file, ${report.skipped} without one, ${report.failures.length} failed`
  );
  if (report.expectedWarnings.length > 0) {
    out.push(`${report.expectedWarnings.length} expected file(s) had keys or values this harness dropped`);
  }
  if (report.scored === 0) {
    out.push("nothing to score: add <id>.expected.json files beside the PDFs (InvoiceFields vocabulary)");
    console.log(out.join("\n"));
    return;
  }
  out.push("");
  printRow(report.table.all, out);
  for (const r of report.table.textClasses) printRow(r, out);
  for (const r of report.table.clusters.slice(0, maxClusters)) printRow(r, out);
  if (report.table.clusters.length > maxClusters) {
    out.push(`… ${report.table.clusters.length - maxClusters} smaller clusters in eval.json`, "");
  }
  if (report.table.slice !== null) printRow(report.table.slice, out);
  console.log(out.join("\n").trimEnd());
}
