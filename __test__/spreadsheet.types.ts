// Type-level assertions over the generated spreadsheet declarations. Not an ava spec:
// checked by `yarn typecheck`. Flip a literal to see it fail.

import type { DocumentFormat, FormatDetection, IsExact, OnlyOptional, PdfDown } from "../index";
import type { detectFormat } from "../index";

true satisfies IsExact<DocumentFormat, "Pdf" | "Xlsx" | "Unknown">;
true satisfies IsExact<ReturnType<typeof detectFormat>, FormatDetection>;
true satisfies IsExact<keyof OnlyOptional<FormatDetection>, "detail">;
true satisfies IsExact<ReturnType<typeof PdfDown.openAsync>, Promise<PdfDown>>;
true satisfies IsExact<PdfDown["format"], DocumentFormat>;
