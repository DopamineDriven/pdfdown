// Type-level assertions over the generated spreadsheet declarations. Not an ava spec:
// checked by `yarn typecheck`. Flip a literal to see it fail.

import type { IsExact, OnlyOptional } from "@d0paminedriven/type-utils";

import type {
  CellBox,
  DocumentFormat,
  FormatDetection,
  NumberSubtype,
  PdfDown,
  SheetCell,
  SheetCells,
  SheetColumn,
  SheetRow,
  SpreadsheetOptions
} from "../index";
import type { detectFormat, detectFormatAsync } from "../index";

true satisfies IsExact<DocumentFormat, "Pdf" | "Xlsx" | "Unknown">;
true satisfies IsExact<ReturnType<typeof detectFormat>, FormatDetection>;
true satisfies IsExact<ReturnType<typeof detectFormatAsync>, Promise<FormatDetection>>;
true satisfies IsExact<keyof OnlyOptional<FormatDetection>, "detail">;
true satisfies IsExact<ReturnType<typeof PdfDown.openAsync>, Promise<PdfDown>>;
true satisfies IsExact<PdfDown["format"], DocumentFormat>;

// `layout` takes the workbook options; a PDF ignores them.
true satisfies IsExact<Parameters<PdfDown["layout"]>, [opts?: SpreadsheetOptions | null | undefined]>;

// Cells are a flat discriminated union on `type`; the typed value is `value`, and Blank has none.
true satisfies IsExact<SheetCell["type"], "Text" | "Number" | "Boolean" | "Error" | "Date" | "Time" | "Blank">;
true satisfies IsExact<Extract<SheetCell, { type: "Number" }>["subtype"], NumberSubtype>;
true satisfies IsExact<NumberSubtype, "Integer" | "Float">;
true satisfies IsExact<Extract<SheetCell, { type: "Time" }>["value"], string>;
true satisfies IsExact<"subtype" extends keyof Extract<SheetCell, { type: "Text" }> ? true : false, false>;
true satisfies IsExact<Extract<SheetCell, { type: "Number" }>["value"], number>;
true satisfies IsExact<Extract<SheetCell, { type: "Date" }>["value"], string>;
true satisfies IsExact<
  keyof OnlyOptional<Extract<SheetCell, { type: "Number" }>>,
  "format" | "formula" | "sharedFormula" | "merge" | "currencySymbol"
>;
true satisfies IsExact<"value" extends keyof Extract<SheetCell, { type: "Blank" }> ? true : false, false>;
true satisfies IsExact<keyof OnlyOptional<SpreadsheetOptions>, "sheets" | "hiddenSheets" | "hiddenCells">;

// Geometry: every cell carries its box in the sheet frame; columns and rows list the exceptions.
true satisfies IsExact<Extract<SheetCell, { type: "Number" }>["box"], CellBox>;
true satisfies IsExact<keyof CellBox, "x0" | "y0" | "x1" | "y1">;
true satisfies IsExact<SheetCells["columns"], Array<SheetColumn>>;
true satisfies IsExact<SheetCells["rows"], Array<SheetRow>>;
true satisfies IsExact<keyof OnlyOptional<SheetCells>, "dimension" | "printArea">;
true satisfies IsExact<keyof SheetColumn, "index" | "width" | "hidden">;
true satisfies IsExact<keyof SheetRow, "index" | "height" | "hidden">;
