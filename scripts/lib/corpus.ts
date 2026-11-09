import { readdirSync } from "fs";
import { basename, extname, join, relative, resolve, sep } from "path";

/**
 * One file of a local corpus. `id` is the basename without its extension: the corpus at hand
 * names every file by a six-digit invoice id, and every per-document output row carries the id
 * so the row traces back to its file.
 */
export interface CorpusFile {
  path: string;
  id: string;
  idValid: boolean;
  kind: "pdf" | "xlsx";
}

const KIND_BY_EXTENSION: Readonly<Record<string, CorpusFile["kind"]>> = { ".pdf": "pdf", ".xlsx": "xlsx" };
const ID_PATTERN = /^\d{6}$/;
const DEFAULT_ROOT = resolve(import.meta.dirname, "..", "..", "ocr", "__test__", "fixtures", "private");

const byId = (a: CorpusFile, b: CorpusFile) =>
  a.id < b.id ? -1 : a.id > b.id ? 1 : a.path < b.path ? -1 : a.path > b.path ? 1 : 0;

/**
 * PDFs and workbooks under `root`, at most one folder deep (a corpus may be grouped by id
 * prefix), sorted by id. Office lock files (`~$…`) and files of any other kind are skipped; a
 * root that is not a directory yields nothing.
 */
export function discoverCorpus(root: string): Array<CorpusFile> {
  let entries;
  try {
    entries = readdirSync(root, { recursive: true, withFileTypes: true });
  } catch {
    return [];
  }
  const files: Array<CorpusFile> = [];
  for (const entry of entries) {
    if (!entry.isFile() || entry.name.startsWith("~$")) continue;
    const extension = extname(entry.name);
    const kind = KIND_BY_EXTENSION[extension.toLowerCase()];
    if (kind === undefined) continue;
    const depth = relative(root, entry.parentPath).split(sep).filter(Boolean).length;
    if (depth > 1) continue;
    const id = basename(entry.name, extension);
    files.push({ path: join(entry.parentPath, entry.name), id, idValid: ID_PATTERN.test(id), kind });
  }
  return files.sort(byId);
}

/**
 * `path` relative to `root` with `/` separators on every platform: output rows carry file paths
 * as identifiers, and an identifier must not change between Linux and Windows.
 */
export function posixRelative(root: string, path: string): string {
  return relative(root, path).split(sep).join("/");
}

/**
 * `$PDFDOWN_INVOICE_CORPUS`, else the repo's gitignored private fixture directory, as an absolute
 * path however it was given.
 */
export function corpusRoot(): string {
  return resolve(relative(process.cwd(), process.env.PDFDOWN_INVOICE_CORPUS ?? DEFAULT_ROOT));
}
