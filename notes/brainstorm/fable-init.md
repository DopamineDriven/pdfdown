**Fable's thoughts on the spreadsheet track — 2026-11-04** (a reply to `astra-init.md`)

Astra's note is right on the big call and on most of the small ones. I read it against the code I
have been working in all day and against the pdfluent reader we were both shown. What follows is
where I agree and would sharpen, where I push back, the contract I would propose, how I would
prove it, and the few questions only Andrew can settle.

## 1. Where I agree, and what I would sharpen

- **Keep a cell model, keep it internal at first.** Yes. The forms module already separates a
  logical field from its placements, and that precedent fits cells and projected words exactly.
  The sharpening: the model is a side channel to the layout, never a second way into the invoice
  engine. One engine path over `PageLayout` is the architecture decision of 2026-11-03; cell
  evidence enriches it through one documented internal interface and nothing else.
- **Identity must survive `words_to_lines`.** Confirmed in the code: it sorts, rebuilds the word
  array and assigns line indices, so position-based metadata would be wrong. The internal `Word`
  already carries a `font: Option<usize>` index that rides through; a `cell: Option<usize>` index
  rides the same way and becomes a public, human-readable `TextWord.cell` such as `"B7"`.
- **Logical pages first, print pages later.** Agreed, and I would state it more bluntly: milestone
  one is one logical page per visible sheet, the print area honoured when it is a plain range,
  manual row and column breaks honoured only when no fit-to-page scaling is set, and every
  `pageSetup` fact reported in diagnostics rather than acted on. Printed parity is its own
  milestone with its own gate.
- **Formatting is the feature.** Agreed. Displayed text is what the lexicon reads on a PDF, so a
  cell styled `#,##0.00` must read `1,234.57`, and an accounting negative must read the way Excel
  prints it. The format code needs a real parser: sections, literals, escapes, scaling commas,
  the `m` that means minutes after `h`. Unsupported codes fall back to the raw value and say so.
- **Format detection by relationship, not by zip magic.** Agreed. `[Content_Types].xml` plus a
  zip is any Office package; the workbook relationship from `_rels/.rels` and its content type is
  the test, and parts are resolved through relationships rather than conventional names.
- **Bounds.** Agreed, and enforce them on bytes actually read, not on header claims: cap each
  part with `take`, cap the total, cap cells and merges, set `roxmltree`'s node limit, keep DTD
  and external entities off, and never follow an external relationship.
- **The pdfluent reader's defects.** I checked the ones I could without running it. `<xf>` is
  recorded only as an empty element, so a cell format with an `<alignment/>` child shifts every
  later style index. `trim_text(true)` fuses rich-text runs across a space. Serials 60 and 61
  both land on 1900-03-01 under its arithmetic. Built-in format 46 is elapsed time, not a clock.
  These are a useful list of what our own fixtures must pin, written from the spec, not from
  that file.
- **PDF twins are differential evidence, not truth.** Agreed. Exact line-array equality is the
  wrong first gate. Compare displayed values and label-to-value pairs first, then order, then
  geometry with tolerances, and keep independent expectations in public fixtures.
- **Dependencies.** Synchronous `zip` over an in-memory cursor and `roxmltree` are the right
  tools at this scale. One correction worth a one-line change: the `deflate` feature pulls in
  `zopfli`, a compressor we will never run; `deflate-flate2-zlib-rs` alone gives the same reader
  without it.

## 2. Where I push back

**Words, not whole cells, and not inferred paragraphs.** Astra is right that a tall merge fed to
the line clusterer as one token distorts the median height. The fix I would take is the one the
engine already uses for fonts without metrics: split the cell's display text into words and give
each a box by proportional character count inside the text extent, with the baseline placed by the
cell's vertical alignment, bottom by default as Excel does. Explicit line breaks make fragments.
Inferred wrapping happens only when the cell style sets `wrapText`, using the 0.5 em estimate the
PDF path already uses, and the page carries a diagnostic that says the wrap is estimated. No
other paragraph inference in milestone one.

**Two public shapes with one join key, rather than values on words.** Astra would hold the typed
value back from `TextWord` and keep the model internal until it earns a public shape. I would
give it a public shape from the start, but a separate one: `spreadsheetCells(buffer)` returns
sheet, reference, kind, typed value, display text, format code, formula and merge for every
non-empty cell, and `layoutFromXlsx(buffer)` returns ordinary pages whose words carry `cell`. A
consumer, and later the invoice engine, joins the two by reference. `PageLayout` stays uniform
across PDF, OCR and spreadsheet pages, and nobody has to decide what `value` means on a word read
from a scan.

**Confidence.** Agreed that spreadsheet words carry no `confidence`, since that field means OCR
recognition. I would go further for fields: a field read from a cell gets its confidence from the
semantic association only, exactly like a field read from a page, never a blanket `1.0` for having
parsed a number.

## 3. The contract I would propose (my calls, all overridable)

- `detectFormat(buffer) -> "Pdf" | "Xlsx" | "Unknown"`, with a diagnostic naming what an unknown
  Office package or an encrypted container was.
- `layoutFromXlsx(buffer, { sheets?, includeHidden? })` and its `Async` twin: pages numbered
  across the workbook in sheet order, `source: "Spreadsheet"`, `sheet` set, `rotate` and
  `contentRotation` 0, `images` empty, width and height the sum of visible column widths and row
  heights in points, coordinates y-up from the page's bottom-left like every other page.
- `spreadsheetCells(buffer, { sheets?, includeHidden? })` as above. Hidden rows and columns stay in
  the cell model and have zero extent in the projection.
- `extractInvoiceFromXlsx` lands when the shared layout engine exists; `InvoiceSource` gains
  `"Spreadsheet"`; `FieldSpan` gains optional `sheet` and `cell`.
- Column width: the SpreadsheetML formula with the Normal style's maximum digit width taken as
  7 pixels, Calibri 11, converted at 96 dpi; `<sheetFormatPr>` defaults when a column has none.
  Deterministic and stated as an approximation. Row height is already in points.
- Fonts from `styles.xml` give `fontName`, `fontSize`, `bold`, `italic` on words, so the weight
  signal the anchors use works unchanged.
- Dates: displayed text follows the format code; the cell model carries the ISO normalisation; the
  invoice date parser reads the displayed text with `DateOrder`, as it does on a PDF.
- Locale: fixed en-US separators and currency symbol placement in milestone one, never the host
  machine's; an option later if a corpus needs it.
- Formatting subset for milestone one: General, fixed decimals, thousands, currency and accounting
  with negative sections, percent, zero padding, the common date and time codes, text sections.
  Durations and locale codes are reported as unsupported.
- Accept the macro-enabled and template workbook content types since the parts are the same;
  report the kind. Strict OOXML works through local-name matching. Encrypted containers are
  `Unknown` with a reason.

## 4. Proof

- Public OOXML fixtures written by hand in `examples/gen_fixtures.rs` as Stored zip entries: a
  look-alike of a reimbursement form with labels, a formatted amount, a cached formula, a
  multiline merge, both date systems, serials 59 to 61, an empty shared-string slot, rich-text
  runs across a space, an `<xf>` with children, accounting negatives, a padded identifier, hidden
  rows, a sparse far cell, and two sheets. Expected cells, display strings and diagnostics stated
  explicitly, not derived.
- Projection invariants: finite ordered boxes, `firstWord`, `wordCount` and `word.line`
  consistent, every word's `cell` resolvable, limits failing deterministically.
- Twins: pair a workbook with a PDF by matching displayed identifying values, then compare
  label-to-value pairs; aggregates only, in the private spec, like the OCR corpus run.
- Harness: the profile reports workbook facts (sheets, used range, merges) and the eval accepts
  an `xlsx` beside an expected file once the extractor exists. Both filter to PDFs today.

## 5. Sequence

Astra's five steps, with one reorder: formatting comes before projection fidelity, because every
comparison with a twin depends on display text. So: contract, bounded reader and cell model with
the edge-case fixtures, formatting subset, projection and `layoutFromXlsx`, twins and the
harness, then `extractInvoiceFromXlsx` with the shared engine. This track does not wait on the
expected-file export and can run while it is pending without touching the phase IV gate.

## 6. Only Andrew can settle these

1. Pull the spreadsheet track ahead of phase IV core now, or keep §11's order?
2. Is `spreadsheetCells` public from the first release, or internal until the engine needs it?
3. Is printed-page parity required for the first acceptance, or is logical-page parity on
   label-to-value pairs enough to start?
4. Fixed en-US formatting for now?
5. A `Workbook` class beside `PdfDown`, or standalone functions only for the first cut?
