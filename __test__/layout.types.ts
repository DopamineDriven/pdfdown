// Type-level assertions over the generated layout declarations. Not an ava spec:
// checked by `yarn typecheck` (tsc --noEmit). Flip a literal to see it fail.

import type {
  IsExact,
  OnlyOptional,
  OnlyRequired,
  PageLayout,
  PlacedImage,
  TextLine,
  TextSource,
  TextWord
} from "../index";
import { layoutPerPage } from "../index";

// `TextSource` is one union in every build; `Rendered` only ever appears at runtime with PDFium.
const _source = true satisfies IsExact<TextSource, "Native" | "Ocr" | "Rendered">;
const _pageSource = true satisfies IsExact<PageLayout["source"], TextSource>;

// Native-only and OCR-only word fields are optional; geometry and the line index never are.
const _wordOptional = true satisfies IsExact<
  keyof OnlyOptional<TextWord>,
  "fontSize" | "fontName" | "bold" | "italic" | "confidence"
>;
const _wordRequired = true satisfies IsExact<keyof OnlyRequired<TextWord>, "text" | "x0" | "y0" | "x1" | "y1" | "line">;

// Page-level fields are all required; contentRotation is a plain number (0 on native pages).
const _pageRequired = true satisfies IsExact<keyof OnlyOptional<PageLayout>, never>;
const _contentRotation = true satisfies IsExact<PageLayout["contentRotation"], number>;

// Lines and placements are fully required.
const _lineRequired = true satisfies IsExact<keyof OnlyOptional<TextLine>, never>;
const _imageRequired = true satisfies IsExact<keyof OnlyOptional<PlacedImage>, never>;

// Boxes share one shape across words, lines and images.
type Box = Pick<TextWord, "x0" | "y0" | "x1" | "y1">;
const _lineBox = true satisfies IsExact<Pick<TextLine, keyof Box>, Box>;
const _imageBox = true satisfies IsExact<Pick<PlacedImage, keyof Box>, Box>;

// The entry point returns one layout per page.
const _returns = true satisfies IsExact<ReturnType<typeof layoutPerPage>, Array<PageLayout>>;
