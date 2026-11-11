// Type-level assertions over the generated layout declarations. Not an ava spec:
// checked by `yarn typecheck`. Flip a literal to see it fail.

import type { IsExact, OnlyOptional, OnlyRequired } from "@d0paminedriven/type-utils";

import type { PageLayout, PlacedImage, TextLine, TextSource, TextWord } from "../index";
import type { layoutPerPage } from "../index";

// `TextSource` is one union in every build; `Rendered` only ever appears at runtime with PDFium,
// and `Spreadsheet` on the logical pages of a workbook.
true satisfies IsExact<TextSource, "Native" | "Ocr" | "Rendered" | "Spreadsheet">;
true satisfies IsExact<PageLayout["source"], TextSource>;

// Native-only, OCR-only and spreadsheet-only word fields are optional; geometry and the line
// index never are.
true satisfies IsExact<
  keyof OnlyOptional<TextWord>,
  "fontSize" | "fontName" | "bold" | "italic" | "symbolic" | "confidence" | "cell"
>;
true satisfies IsExact<keyof OnlyRequired<TextWord>, "text" | "x0" | "y0" | "x1" | "y1" | "line">;

// Page-level fields are required apart from the workbook sheet name; contentRotation is a plain
// number (0 on native pages).
true satisfies IsExact<keyof OnlyOptional<PageLayout>, "sheet">;
true satisfies IsExact<PageLayout["contentRotation"], number>;

// Lines and placements are fully required.
true satisfies IsExact<keyof OnlyOptional<TextLine>, never>;
true satisfies IsExact<keyof OnlyOptional<PlacedImage>, never>;

// Boxes share one shape across words, lines and images.
type Box = Pick<TextWord, "x0" | "y0" | "x1" | "y1">;
true satisfies IsExact<Pick<TextLine, keyof Box>, Box>;
true satisfies IsExact<Pick<PlacedImage, keyof Box>, Box>;

// The entry point returns one layout per page.
true satisfies IsExact<ReturnType<typeof layoutPerPage>, PageLayout[]>;
