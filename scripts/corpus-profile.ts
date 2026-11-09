// Profile a local PDF corpus: `yarn corpus:profile [--verbose]`.
//
// Reads the gitignored private fixture directory (`ocr/__test__/fixtures/private`), runs every
// public extractor once per PDF, writes `<root>/profile.json` next to the PDFs and prints the
// aggregate tables of notes/2026-11-03/1037.md §2. Nothing from the corpus is printed unless
// `--verbose` names the files whose basename is not a six-digit id or that failed to parse.
import { writeFileSync } from "fs";
import { join } from "path";

import { corpusRoot, discoverCorpus } from "./lib/corpus";
import { printProfile, profileCorpus } from "./lib/profile";

const verbose = process.argv.includes("--verbose");
const root = corpusRoot();
const files = discoverCorpus(root);
if (files.length === 0) {
  console.error(`no PDFs or workbooks under ${root}`);
  process.exit(1);
}

const list = (ids: Array<string>) => (verbose ? `: ${ids.join(", ")}` : " (--verbose lists them)");
const invalid = files.filter((f) => !f.idValid);
if (invalid.length > 0) {
  console.warn(`${invalid.length} file(s) whose basename is not a six-digit id${list(invalid.map((f) => f.id))}`);
}

const profile = profileCorpus(files, root);
const out = join(root, "profile.json");
writeFileSync(out, `${JSON.stringify(profile, null, 2)}\n`);
printProfile(profile);
if (profile.failures.length > 0) {
  console.warn(`${profile.failures.length} file(s) failed to parse${list(profile.failures.map((f) => f.id))}`);
}
console.log(`\nwrote ${out}`);
