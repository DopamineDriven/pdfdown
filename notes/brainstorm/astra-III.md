**Astra's workbook inspection — 2026-11-04**

I inspected all 17 XLSX files Andrew identified under the private `343`, `344`, and `345` directories. All were accessible. These samples support starting with a small, bounded workbook reader behind the unified `PdfDown` class, and they make several formatting and provenance requirements concrete.

This was a read-only ZIP/XML inspection using Python's standard library. I followed package relationships to the workbook, worksheets, styles, and shared strings, inspected the stored data and metadata, and collected aggregate counts. I did not evaluate formulas, render the workbooks, establish PDF pairs, or run an XLSX implementation in this repository. The note records structural facts and generic format codes; it does not reproduce private cell values, formulas, sheet names, or document identifiers. Temporary probe code and reports are under `/tmp/astra_xlsx_*`.

**The samples are small and structurally consistent, but formatting is heavily used.**

| Property                        | Observed across the 17 workbooks                                                            |
| ------------------------------- | ------------------------------------------------------------------------------------------- |
| File size                       | 16,336–23,352 bytes each; 303,078 bytes total                                               |
| ZIP contents                    | 11–15 entries each; all 244 entries use Deflate                                             |
| Total uncompressed archive size | 58,386–108,693 bytes per workbook                                                           |
| Largest individual part         | 55,700 bytes across the sample                                                              |
| Sheets                          | Exactly one visible worksheet per workbook; one shared sheet-name value across the sample   |
| XML namespace and date system   | Transitional SpreadsheetML; 1900 date system throughout                                     |
| Serialized rows/cells           | 51–135 rows and 672–1,460 cell records per workbook                                         |
| Value/formula cells             | 92–455 per workbook; 2,500 total                                                            |
| Empty cell records              | 12,143 total, including substantial style/layout structure                                  |
| Merges                          | 98–187 per workbook; 2,158 total                                                            |
| Multirow merges                 | Exactly one two-row merge per workbook                                                      |
| Shared strings                  | 54–113 entries per workbook; no rich-text runs or empty string-table entries observed       |
| Explicit newlines               | One shared-string entry containing a newline in every workbook                              |
| Hidden dimensions               | Two hidden rows in one workbook; no hidden column definitions                               |
| Images and external links       | No media parts, drawing references, external-link parts, or external relationships observed |

“Value/formula cell” includes a formula with a cached empty-string result. It does not mean that every such cell emits visible text. Of the 2,500 cells, 2,231 contain a nonempty stored value and 269 have an explicitly empty string cache.

These sizes make the existing `roxmltree` a reasonable starting point. They do not establish general workbook limits or a production performance benchmark. Keep the bounds and generic parser design from the earlier notes.

**The nested-style issue affects every value/formula cell in this sample.** There are 1,053 `cellXfs/xf` records across the workbooks, of which 853 contain child elements. Every one of the 2,500 value/formula cells references an `xf` with children. The reference reader's empty-element-only handling would therefore miss the actual styles used by these cells. This needs a fixture and correct parsing in the first implementation slice.

Alignment also deserves early attention. Among those 2,500 cells, 2,499 explicitly specify vertical centering, 1,164 have `wrapText=1`, and 1,389 have an indentation value of 1. Horizontal alignment varies between left, right, and center. A layout adapter that only places text at the bottom-left of each cell will lose information already present in these workbooks.

The Normal style is Calibri 11 in all 17 files, while every value/formula cell uses Arial, at sizes from 8 to 26 points. This distinction matters: the Normal font participates in column-width interpretation, while the cell's effective font governs its text estimate. Fable's proposed Calibri-11 width approximation has a concrete basis for this sample; it should not make all projected text use Calibri metrics.

**Shared formulas and string caches are immediate requirements.** The stored formula results break down as follows:

| Formula property                                   | Count                    |
| -------------------------------------------------- | ------------------------ |
| Formula cells                                      | 837                      |
| Shared-formula records                             | 723, in every workbook   |
| Shared masters with expression and range           | 118                      |
| Shared followers without their own expression text | 605                      |
| Ordinary formula records                           | 114                      |
| Numeric caches                                     | 455                      |
| Nonempty string caches                             | 113                      |
| Explicitly empty string caches                     | 269, across 16 workbooks |
| Missing `<v>` elements                             | 0                        |
| Error caches                                       | 0                        |

Every empty cache in this sample has `t="str"`. Represent these as cached empty strings. Treating them as zero or automatically reporting them as uncalculated would misdescribe the file. The missing-cache case still needs a separate synthetic fixture.

The public cell model must also distinguish “formula present” from “formula expression text stored on this cell.” Shared followers have cached results but refer to an expression stored on a master. For the first release, preserve shared-group identity and master/range information without recalculating formulas. If the API promises a full expression for every cell, relative-reference expansion becomes an additional implementation requirement; otherwise document that the raw formula representation can reference a shared master.

**Cell type must take precedence over number format.** All 113 nonempty formula-string results use built-in number format 4, and another 67 shared-string cells use built-in date format 14. A number or date style does not make these values numeric. Resolve `t="s"` through the string table and preserve `t="str"` results as text before applying any numeric-format interpretation.

For numeric values only, including numeric formula caches, the active formats are:

| Format               | Numeric cells using it |
| -------------------- | ---------------------- |
| `#,##0.00"/hr"`      | 277                    |
| `0.0`                | 195                    |
| Built-in 4           | 216                    |
| `mm/dd/yy`           | 68                     |
| Built-in 14          | 17                     |
| Built-in 2           | 1                      |
| General / built-in 0 | 4                      |

The literal `/hr` suffix is especially useful evidence for the formatter design. Preserve it in display text while retaining the numeric value separately. It conveys a unit-bearing rate; it must not accidentally turn a numeric format into a clock-time format because the literal contains `h`.

One workbook defines a dollar accounting format, but none of the value/formula cells uses it. Accounting behavior still belongs in the intended supported subset and synthetic tests, but this sample does not validate it. The same caution applies to percentages, elapsed durations, the 1904 system, rich text, multiple sheets, and Strict OOXML: the samples do not exercise those features.

These files make a deterministic initial formatting subset practical. They do not justify overriding explicit format codes with US conventions. Preserve the literal suffix and explicit date ordering, and use a documented fallback only where the supported format leaves a choice. Typed numeric dates should retain their normalized interpretation through invoice extraction; date-styled string cells still require text interpretation if their semantic role calls for it.

**Print-area selection is practical here; exact print pagination remains a separate task.** Every workbook has one rectangular print area beginning at A1 and ending in column O. Every workbook sets `fitToPage="1"` and `fitToHeight="0"`; none stores an explicit `fitToWidth` attribute. There are no row/column break records, print-title definitions, or nonempty header/footer text elements in these samples. All report portrait orientation and the same margins.

The absence of stored break records does not establish a single printed page. A print implementation still has to interpret defaults, fit settings, and page dimensions. I continue to recommend a logical page per selected sheet/print range for the first milestone, with a clearly named coordinate contract.

In 16 workbooks, the declared worksheet dimension matches the print area. The remaining workbook declares `A1:S135` while its print area is `A1:O97`. Its 119 serialized cells outside the print area are all empty: no value or formula cell falls outside the selected print area. This corrects the preliminary possibility raised during inspection that populated data might be omitted. It also demonstrates why using the dimension alone can unnecessarily enlarge a page.

The two hidden rows contain two nonempty values, so visibility filtering is a real requirement even though the sample has no hidden sheets or columns. Preserve those cells in the workbook model, and make their inclusion in cell results and visible layout explicit. Among the 1,305 nonempty stored values located inside merged ranges, all occupy the merge anchor; no populated non-anchor merge cells were observed.

**The evidence strengthens the earlier recommendations.** I would bring the bounded reader, public cell results, formatter, and logical projection forward now. The class should open either format asynchronously, retain parsed state, and expose workbook cells and layout from that same instance. Nothing in the sample requires a second public class, a streaming ZIP reader, or formula evaluation.

I would prioritize these synthetic cases before a private-corpus acceptance run:

1. Nested `xf` records, a Calibri Normal style with Arial cell text, centered vertical alignment, indentation, and an explicit newline in a two-row merge.
2. A shared-formula master and followers with numeric, nonempty string, and empty-string caches. Add a separate missing-cache case that is absent from the real sample.
3. The `/hr` literal format, decimal formats, supported dates, and string cells assigned numeric/date styles. Expected values and display strings should be invented and explicit.
4. A print area smaller than the declared dimension, empty styled cells beyond it, and hidden rows containing values.
5. Deflate-compressed input, since every supplied archive uses it. Stored-only fixture generation would miss the actual compression path exercised here.

Keep the broader adversarial and format-variation fixtures from the earlier notes as well. These 17 workbooks provide direct acceptance material for one consistent form family; they cannot alone prove general spreadsheet support. PDF pairing and semantic label-to-value comparisons remain separate work, and the shared layout-based invoice engine is still a dependency of spreadsheet invoice extraction.
