import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join, resolve } from "path";

import test from "ava";

import { corpusRoot, discoverCorpus } from "../lib/corpus";

/** A corpus laid out the way the private one is: id-prefix folders, a stray note, a lock file. */
const scaffold = () => {
  const root = mkdtempSync(join(tmpdir(), "pdfdown-corpus-"));
  mkdirSync(join(root, "a"));
  mkdirSync(join(root, "b", "deeper"), { recursive: true });
  writeFileSync(join(root, "a", "1.pdf"), "%PDF-1.4\n");
  writeFileSync(join(root, "b", "2.PDF"), "%PDF-1.4\n");
  writeFileSync(join(root, "b", "notes.md"), "# notes\n");
  writeFileSync(join(root, "b", "deeper", "9.pdf"), "%PDF-1.4\n");
  writeFileSync(join(root, "3.xlsx"), "PK");
  writeFileSync(join(root, "~$4.xlsx"), "lock");
  writeFileSync(join(root, "123456.pdf"), "%PDF-1.4\n");
  return root;
};

test("corpus discovery: one level of id folders, any-case extensions, no notes, lock files or deeper levels", (t) => {
  const root = scaffold();
  try {
    const files = discoverCorpus(root);
    t.deepEqual(
      files.map((f) => `${f.id} (${f.kind})`),
      ["1 (pdf)", "123456 (pdf)", "2 (pdf)", "3 (xlsx)"],
      "sorted by id; notes.md, the ~$ lock file and the depth-2 pdf are not corpus files"
    );
    t.deepEqual(
      files.map((f) => f.idValid),
      [false, true, false, false],
      "only a six-digit basename is a valid id"
    );
    t.true(
      files.every((f) => f.path.startsWith(root) && f.path.toLowerCase().endsWith(f.kind === "pdf" ? ".pdf" : ".xlsx")),
      "paths are absolute and keep the file's own extension case"
    );
    t.is(discoverCorpus(join(root, "a", "1.pdf")).length, 0, "a file is not a corpus");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("corpus root: the gitignored private directory, as an absolute path", (t) => {
  t.is(corpusRoot(), resolve(import.meta.dirname, "..", "..", "ocr", "__test__", "fixtures", "private"));
});
