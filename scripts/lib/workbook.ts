// Workbook facts for the corpus profile, read through the public API only: `spreadsheetCells`
// for the cells and `layoutFromXlsx` for the logical pages. Nothing here opens the package
// itself; what the profile reports is what a consumer can see.

import { readFileSync } from "fs";
import { relative } from "path";

import { layoutFromXlsx, spreadsheetCells } from "../../index";
import type { SheetCell } from "../../index";
import type { CorpusFile } from "./corpus";

export type CellType = SheetCell["type"];
export const CELL_TYPES = ["Text", "Number", "Boolean", "Error", "Date", "Time", "Blank"] as const;

/** Per-workbook facts. `id` first: every row traces back to its file. */
export interface WorkbookEntry {
  id: string;
  file: string;
  bytes: number;
  sheets: number;
  hiddenSheets: number;
  /** Each sheet's declared dimension in A1 form, `null` when a sheet states none. */
  dimensions: Array<string | null>;
  cells: number;
  merges: number;
  cellTypes: Record<CellType, number>;
  /** Format code -> number of Number and Date cells styled with it. */
  formats: Record<string, number>;
  /** Cells whose format this release does not render (`formatSupported: false`). */
  unsupportedFormats: number;
  /** Cells carrying their own formula; followers of a shared formula are counted separately. */
  formulas: number;
  sharedFormulaFollowers: number;
  cellWarnings: number;
  /** Logical pages of the visible sheets, with their sizes in points. */
  pages: number;
  pageSizes: Array<{ width: number; height: number }>;
  layoutMs: number;
  layoutWarnings: number;
}

/** A workbook the reader refused; it still carries its id. */
export interface WorkbookFailure {
  id: string;
  file: string;
  bytes: number;
  kind: "xlsx";
  error: string;
}

const zeroCellTypes = (): Record<CellType, number> => ({
  Text: 0,
  Number: 0,
  Boolean: 0,
  Error: 0,
  Date: 0,
  Time: 0,
  Blank: 0
});

/** Open one workbook through the public API and fold its sheets into a `WorkbookEntry`. */
export function profileWorkbook(file: CorpusFile, root: string): WorkbookEntry | WorkbookFailure {
  const buf = readFileSync(file.path);
  const base = { id: file.id, file: relative(root, file.path), bytes: buf.length };
  try {
    const sheets = spreadsheetCells(buf, { hiddenSheets: true });
    const started = performance.now();
    const pages = layoutFromXlsx(buf);
    const layoutMs = performance.now() - started;
    const cellTypes = zeroCellTypes();
    const formats: Record<string, number> = {};
    let unsupportedFormats = 0;
    let formulas = 0;
    let sharedFormulaFollowers = 0;
    for (const sheet of sheets) {
      for (const cell of sheet.cells) {
        cellTypes[cell.type] += 1;
        if (cell.format !== undefined && (cell.type === "Number" || cell.type === "Date")) {
          formats[cell.format] = (formats[cell.format] ?? 0) + 1;
        }
        if (!cell.formatSupported) unsupportedFormats += 1;
        if (cell.formula !== undefined) formulas += 1;
        if (cell.sharedFormula !== undefined) sharedFormulaFollowers += 1;
      }
    }
    return {
      ...base,
      sheets: sheets.length,
      hiddenSheets: sheets.filter((s) => s.hidden).length,
      dimensions: sheets.map((s) => s.dimension ?? null),
      cells: sheets.reduce((n, s) => n + s.cells.length, 0),
      merges: sheets.reduce((n, s) => n + s.merges.length, 0),
      cellTypes,
      formats,
      unsupportedFormats,
      formulas,
      sharedFormulaFollowers,
      cellWarnings: sheets.reduce((n, s) => n + s.warnings.length, 0),
      pages: pages.length,
      pageSizes: pages.map((p) => ({ width: Math.round(p.width * 10) / 10, height: Math.round(p.height * 10) / 10 })),
      layoutMs,
      layoutWarnings: pages.reduce((n, p) => n + p.warnings.length, 0)
    };
  } catch (error) {
    return { ...base, kind: "xlsx", error: error instanceof Error ? error.message : String(error) };
  }
}
