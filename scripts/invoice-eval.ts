// Evaluate `extractInvoice` against expected files: `yarn invoice:eval [--ids <file>] [--verbose]`.
//
// For every `<id>.pdf` under `$PDFDOWN_INVOICE_CORPUS` (default: the gitignored private fixture
// directory) that has an `<id>.expected.json` beside it, written in pdfdown's own `InvoiceFields`
// vocabulary, the extraction is scored field by field; documents are grouped by the header
// fingerprint in `<root>/profile.json` when `yarn corpus:profile` has written one. Results go to
// `<root>/eval.json`; stdout carries aggregates only. `--ids <file>` (one id per line) adds a row
// for that slice; `--verbose` names the ids whose expected file was partly dropped or that failed.
import { existsSync, readFileSync, writeFileSync } from "fs";
import { join } from "path";

import { corpusRoot, discoverCorpus } from "./lib/corpus";
import { evaluateCorpus, printEval, profileLookup } from "./lib/evaluate";

const args = process.argv.slice(2);
const verbose = args.includes("--verbose");
const idsAt = args.indexOf("--ids");
const idsFile = idsAt === -1 ? undefined : args[idsAt + 1];
if (idsAt !== -1 && idsFile === undefined) {
  console.error("--ids needs a file with one id per line");
  process.exit(2);
}
const ids =
  idsFile === undefined
    ? undefined
    : new Set(
        readFileSync(idsFile, "utf8")
          .split(/\r?\n/)
          .map((l) => l.trim())
          .filter((l) => l !== "" && !l.startsWith("#"))
      );

const root = corpusRoot();
const files = discoverCorpus(root);
if (files.length === 0) {
  console.error(`no PDFs under ${root}`);
  process.exit(1);
}
const profilePath = join(root, "profile.json");
const lookup = existsSync(profilePath) ? profileLookup(JSON.parse(readFileSync(profilePath, "utf8"))) : undefined;
if (lookup === undefined)
  console.warn("no profile.json beside the corpus; run `yarn corpus:profile` first to group by template cluster");

const report = evaluateCorpus(files, root, { ids, lookup });
const out = join(root, "eval.json");
writeFileSync(out, `${JSON.stringify(report, null, 2)}\n`);
printEval(report);
const list = (entries: Array<string>) => (verbose ? `: ${entries.join(", ")}` : " (--verbose lists them)");
if (report.expectedWarnings.length > 0) {
  console.warn(
    `${report.expectedWarnings.length} expected file(s) partly dropped${list(report.expectedWarnings.map((w) => w.id))}`
  );
}
if (report.failures.length > 0) {
  console.warn(`${report.failures.length} document(s) failed${list(report.failures.map((f) => f.id))}`);
}
console.log(`\nwrote ${out}`);
