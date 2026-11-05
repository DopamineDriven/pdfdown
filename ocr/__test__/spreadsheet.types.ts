// Type-level assertions over the generated spreadsheet declarations. Not an ava spec:
// checked by `yarn typecheck`. Flip a literal to see it fail.

import type { DocumentFormat, FormatDetection, IsExact, OnlyOptional, PdfDown, SheetCell, SpreadsheetOptions } from "../index";
import type { detectFormat } from "../index";

true satisfies IsExact<DocumentFormat, "Pdf" | "Xlsx" | "Unknown">;
true satisfies IsExact<ReturnType<typeof detectFormat>, FormatDetection>;
true satisfies IsExact<keyof OnlyOptional<FormatDetection>, "detail">;
true satisfies IsExact<ReturnType<typeof PdfDown.openAsync>, Promise<PdfDown>>;
true satisfies IsExact<PdfDown["format"], DocumentFormat>;

// `layout` takes the workbook options; a PDF ignores them.
true satisfies IsExact<Parameters<PdfDown["layout"]>, [opts?: SpreadsheetOptions | null | undefined]>;

// Cells are a flat discriminated union on `type`; the typed value is `value`, and Blank has none.
true satisfies IsExact<SheetCell["type"], "Text" | "Number" | "Boolean" | "Error" | "Date" | "Blank">;
true satisfies IsExact<Extract<SheetCell, { type: "Number" }>["value"], number>;
true satisfies IsExact<Extract<SheetCell, { type: "Date" }>["value"], string>;
true satisfies IsExact<keyof OnlyOptional<Extract<SheetCell, { type: "Number" }>>, "format" | "formula" | "sharedFormula" | "merge">;
true satisfies IsExact<"value" extends keyof Extract<SheetCell, { type: "Blank" }> ? true : false, false>;
true satisfies IsExact<keyof OnlyOptional<SpreadsheetOptions>, "sheets" | "hiddenSheets" | "hiddenCells">;
