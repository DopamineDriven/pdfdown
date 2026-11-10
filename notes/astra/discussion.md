#### Discussion from chatgpt earlier tonight with Astra xhigh reasoning

---

Andrew (me): can you please go through the following github files in the spreadsheet directory (src/core/spreadsheet/\*) and let me know if you spot anything that Fable may have overlooked insofar as exhaustively parsing and preserving the structure (and any formulae) from xlsx files?

[https://raw.githubusercontent.com/DopamineDriven/pdfdown/refs/heads/main/src/core/spreadsheet/format.rs](https://raw.githubusercontent.com/DopamineDriven/pdfdown/refs/heads/main/src/core/spreadsheet/format.rs)

[https://raw.githubusercontent.com/DopamineDriven/pdfdown/refs/heads/main/src/core/spreadsheet/mod.rs](https://raw.githubusercontent.com/DopamineDriven/pdfdown/refs/heads/main/src/core/spreadsheet/mod.rs)

[https://raw.githubusercontent.com/DopamineDriven/pdfdown/refs/heads/main/src/core/spreadsheet/package.rs](https://raw.githubusercontent.com/DopamineDriven/pdfdown/refs/heads/main/src/core/spreadsheet/package.rs)

[https://raw.githubusercontent.com/DopamineDriven/pdfdown/refs/heads/main/src/core/spreadsheet/project.rs](https://raw.githubusercontent.com/DopamineDriven/pdfdown/refs/heads/main/src/core/spreadsheet/project.rs)

[https://raw.githubusercontent.com/DopamineDriven/pdfdown/refs/heads/main/src/core/spreadsheet/workbook.rs](https://raw.githubusercontent.com/DopamineDriven/pdfdown/refs/heads/main/src/core/spreadsheet/workbook.rs)

<aside>Note: these same files can be found in src/core/spreadsheet/* in this codebase</aside>

---

Astra: **Yes. There are substantial gaps if the goal is exhaustive structural and formula preservation.** The implementation handles common cell extraction, but several valid XLSX constructs either lose their meaning or disappear from the output.

I reviewed all five files at [`main`, commit `5f9dfaf`](https://github.com/DopamineDriven/pdfdown/commit/5f9dfaf9f92f7996ba1fc7993d09f309541dd8f5), plus the public spreadsheet types and tests. I also ran an isolated Rust harness using the actual parser, formatter, and projection code: **21 existing tests passed, and 16 targeted counterexamples demonstrated fidelity problems.** Those include deliberate limitations as well as bugs.

**1. Formula preservation needs a richer representation.**

The main problem is in [`workbook.rs`, formula parsing](https://github.com/DopamineDriven/pdfdown/blob/5f9dfaf9f92f7996ba1fc7993d09f309541dd8f5/src/core/spreadsheet/workbook.rs#L771-L824). It preserves formula text and handles shared-master references, but discards most other formula attributes.

| Formula construct | What currently survives | What disappears |
|---|---|---|
| Ordinary formula | Expression and cached value | Recalculation flags |
| Shared formula | Master expression; follower’s master-cell address | Shared group index and master range |
| Array formula | Anchor expression | Formula type, array range, membership |
| What-if data-table formula | Cached value, when present | Formula identity, range, input references, dimensionality |
| Dynamic-array formula | Expression when present in `<f>` | Associated metadata and spill semantics |

These distinctions are part of the file format; preserving the expression alone does not preserve the formula’s meaning. [Microsoft Learn](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.spreadsheet.cellformula?view=openxml-3.0.1\&utm_source=chatgpt.com)

For example:

```xml
<c r="B1">
  <f t="dataTable" ref="B1:C5" r1="A1"/>
  <v>42</v>
</c>
```

**I confirmed that this produces exactly the same internal model as a literal cell containing `42`.** With no cached `<v>`, that formula cell can disappear entirely because the empty `<f>` is not recognized as a formula.

Likewise, an array formula and an ordinary formula containing the same expression produce identical models. Changing a shared master’s range from `A1:A3` to `A1:A9` produces no model change.

I would preserve a formula object containing its **kind, original expression, range, shared-group information, and original attributes**. Formula evaluation can remain outside the parser.

**2. Formulas and dependencies outside worksheet cells are largely omitted.**

[`parse_workbook()`](https://github.com/DopamineDriven/pdfdown/blob/5f9dfaf9f92f7996ba1fc7993d09f309541dd8f5/src/core/spreadsheet/workbook.rs#L475-L594) handles only `_xlnm.Print_Area` among defined names. [`parse_sheet()`](https://github.com/DopamineDriven/pdfdown/blob/5f9dfaf9f92f7996ba1fc7993d09f309541dd8f5/src/core/spreadsheet/workbook.rs#L644-L687) does not process table parts, validation rules, or conditional-formatting rules.

Consequently:

| Source structure | Information not preserved |
|---|---|
| Defined names | Named ranges, constants, named formulas, sheet-local scope |
| Excel tables | Table names, boundaries, column definitions, calculated-column formulas, totals-row formulas |
| External workbook links | The mapping needed to interpret external references |
| Data validation | Constraints, list sources, `formula1` and `formula2` |
| Conditional formatting | Rule formulas, applicable ranges, priorities, differential styles |
| Charts | Series references/formulas and chart structure |
| Cell metadata | Dynamic-array and rich-value associations |

Named formulas and calculated-column formulas are real formula storage locations, not merely presentation metadata. [Microsoft Learn](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.spreadsheet.definednames?view=openxml-3.0.1\&utm_source=chatgpt.com)

For example, `SUM(Parts[Cost])*TaxRate` can survive as a string while the definitions of **both `Parts` and `TaxRate` are discarded**.

That is probably the biggest architectural gap for your goal.

**3. Styled blank cells are deliberately discarded, which loses structure.**

In [`resolve_cell()`](https://github.com/DopamineDriven/pdfdown/blob/5f9dfaf9f92f7996ba1fc7993d09f309541dd8f5/src/core/spreadsheet/workbook.rs#L856-L864), a cell without a value or recognized formula returns `None`.

```xml
<c r="A1" s="5"/>
```

That cell disappears. The existing [TypeScript test explicitly requires this behavior](https://github.com/DopamineDriven/pdfdown/blob/5f9dfaf9f92f7996ba1fc7993d09f309541dd8f5/__test__/spreadsheet.spec.ts#L135).

For text extraction, that can be an intentional optimization. For structural preservation, blank cells can carry:

- Borders defining a form or table.
- Fill and protection settings identifying input areas.
- Number formats describing expected values.
- Styles belonging to merged regions.

Preserve explicitly stored blank cells sparsely; there is no need to materialize the entire Excel grid.

There is a related geometry bug: [`content_range()`](https://github.com/DopamineDriven/pdfdown/blob/5f9dfaf9f92f7996ba1fc7993d09f309541dd8f5/src/core/spreadsheet/project.rs#L195-L215) considers merges only through retained anchor cells. **A sheet containing an empty `A1:D4` merge retains that range in `merges`, but reports a zero-size frame.** I reproduced this.

**4. Style preservation is substantially incomplete.**

[`parse_styles()`](https://github.com/DopamineDriven/pdfdown/blob/5f9dfaf9f92f7996ba1fc7993d09f309541dd8f5/src/core/spreadsheet/workbook.rs#L268-L352) reads a subset of fonts, number formats, and alignment.

It does not preserve borders, fills, font colours, underline/strike/superscript, protection, themes, differential styles, or the full alignment settings. It also ignores `cellStyleXfs`, `xfId`, and `apply*` attributes, which participate in interpreting styles. [Microsoft Learn](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.spreadsheet.cellformat?view=openxml-3.0.1\&utm_source=chatgpt.com)

Additionally:

- Row and column style metadata is omitted.
- Rich-text runs are concatenated, losing their individual formatting.
- The public cell representation does not expose the internal style index or a reusable style table.

For context-agnostic extraction, borders, fills, and rich-text emphasis can express structure even when the cells contain no explanatory labels.

**5. Missing formula results can become fabricated values.**

In [`resolve_cell()`](https://github.com/DopamineDriven/pdfdown/blob/5f9dfaf9f92f7996ba1fc7993d09f309541dd8f5/src/core/spreadsheet/workbook.rs#L825-L865):

```xml
<c r="A1" t="b">
  <f>1=1</f>
</c>
```

produces **`Boolean(false)` with `cached: false`**.

The formula has no stored result. It should expose a missing result, rather than a Boolean value inferred from an empty string.

There are related fidelity issues:

- `<v/>` counts as cached because the element exists, regardless of its contents.
- Workbook calculation settings and formula recalculation flags are discarded.
- A Boolean stored as `<v>1</v>` becomes `raw: "TRUE"`, so `raw` does not consistently mean original stored spelling.

Cached values represent previously calculated results; their presence does not establish freshness. [Microsoft Learn](https://learn.microsoft.com/en-us/office/open-xml/spreadsheet/working-with-formulas?utm_source=chatgpt.com)

I would separate **formula metadata, cached-result presence/type/value, and calculation settings**.

**6. Some formats are accepted as supported but rendered incorrectly.**

These were reproduced against `format.rs`:

| Input | Expected display | Current display |
|---|---|---|
| `123456789` with `000-00-0000` | `123-45-6789` | `123456789--` |
| August date with `mmmmm` | `A` | `August` |
| `0.75` with `h:mm A/P` | `6:00 P` | `6:00 PM` |
| Text `ABC` with `0;0;0;"ID: "@` | `ID: ABC` | `ABC` |

Microsoft documents these formatting behaviors. [Microsoft Support](https://support.microsoft.com/en-us/excel/keeping-leading-zeros-and-large-numbers?utm_source=chatgpt.com)

The causes are identifiable:

- [`render_number()`](https://github.com/DopamineDriven/pdfdown/blob/5f9dfaf9f92f7996ba1fc7993d09f309541dd8f5/src/core/spreadsheet/format.rs#L493-L553) emits the entire number at the first placeholder, then appends intervening literals.
- All month runs of four or more characters become `MonthName`.
- `A/P` and `AM/PM` become the same token.
- Text cells bypass formatting, so the fourth section is never applied.

These differ from the **explicitly unsupported** scientific, fractional, elapsed-duration, conditional, and locale formats. Unsupported formats generally retain raw values and report limitations; the examples above produce incorrect text while claiming support.

**7. String decoding and explicit date typing have gaps.**

[`rich_text()`](https://github.com/DopamineDriven/pdfdown/blob/5f9dfaf9f92f7996ba1fc7993d09f309541dd8f5/src/core/spreadsheet/workbook.rs#L354-L382) performs XML text extraction but does not decode OOXML’s additional `_xHHHH_` escapes.

I confirmed:

- `one_x000D_two` stays literal instead of containing a carriage return.
- `_x005F_x000D_` stays unchanged instead of becoming the literal text `_x000D_`.

This requires a careful single-pass decoder, including escaped underscores. Ordinary XML entity decoding is insufficient. [Microsoft Learn](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/d34ae755-c53f-4a44-a363-c6dd3ee018a4?utm_source=chatgpt.com)

Also, `t="d"` has no dedicated branch. An explicitly typed ISO date becomes `CellValue::Text`; I reproduced that too. SpreadsheetML explicitly supports this representation. [Microsoft Learn](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.spreadsheet.cellvalue?utm_source=chatgpt.com)

**8. Geometry and layout are lossy derived outputs.**

Two concrete problems in `project.rs`:

- **Fixed row heights are changed.** A row with `ht="10" customHeight="1"` containing two text lines becomes `26.4` points. [`build()`](https://github.com/DopamineDriven/pdfdown/blob/5f9dfaf9f92f7996ba1fc7993d09f309541dd8f5/src/core/spreadsheet/project.rs#L409-L428) always grows rows, and `customHeight` was never retained.
- **Words can disappear at the frame boundary.** A single default-width A1 containing `alpha beta gamma delta epsilon` produces only `alpha beta` in layout output. [`place_cell()`](https://github.com/DopamineDriven/pdfdown/blob/5f9dfaf9f92f7996ba1fc7993d09f309541dd8f5/src/core/spreadsheet/project.rs#L477-L484) drops words whose clipped rectangles have zero width.

The complete string survives in `spreadsheetCells`; the second problem specifically affects the layout path.

Other deliberate projection choices also matter: print areas crop content, hidden sheets are excluded by default, hidden cells are excluded from layout by default, and drawings are not projected. **The layout output cannot serve as the exhaustive representation.**

**9. Additional package and workbook coverage limits need to be explicit.**

In [`package.rs`](https://github.com/DopamineDriven/pdfdown/blob/5f9dfaf9f92f7996ba1fc7993d09f309541dd8f5/src/core/spreadsheet/package.rs#L111-L122), XML is assumed to be UTF-8. A UTF-16 XML part fails; I reproduced that. OPC supports UTF-8 and UTF-16 XML. [Microsoft Learn](https://learn.microsoft.com/en-us/previous-versions/windows/desktop/opc/package-consumption-error-group?utm_source=chatgpt.com)

Other omissions visible in the code include:

- Strict OOXML `.xlsx` files are explicitly rejected.
- Worksheet relationships are not traversed to preserve hyperlinks, comments, drawings, and related parts.
- Sheet relationship types are not used to distinguish worksheets from chartsheets.
- `hidden` and `veryHidden` collapse into one Boolean.
- Outline groups, frozen panes, filters, print titles, headers/footers, and page-break settings are omitted.
- Unrecognized elements generally fall through `_ => {}` without a coverage warning.

An empty warnings array therefore does **not** mean the workbook was exhaustively understood.

My implementation priority would be **formula metadata and dependencies first, explicit blank cells and styles second, then the verified decoding/formatting defects**. I would make the preserved workbook model authoritative, with display strings and page geometry derived from it. For features you have not implemented yet, retain their source representation or report precisely what was omitted.

The verification here covered the Rust core in an isolated harness, with N-API annotations removed; I did not run the full Node/WASM package suite.

---

Andrew: I know it's missing image support (despite pdfdown handling images incredibly well for pdfs by decompressing and surfacing them as buffers by page number with full metadata (width height etc) intact) and I mention this because some xlsx files I probed today have company logos in the actual xlsx and graphics will likely continue to manifest going forward...not to mention *charts*  -- that's incredibly common in xlsx files -- graphs/charts/etc

should I make a rust monorepo? because I have a `@d0paminedriven/docdown` package I just made which is also rust based in addition to `@d0paminedriven/videodown` and `@d0paminedriven/audiodown`


that might make it easier to consume my own crates than isolated singleton repos

....and also I think xlsx should go in the docdown repo (its purpose being for parsing (phase I) xlsx, docx, and pptx in particular, followed by implementing handling for google open format equivalents once solid.  pdfdown is, in a nutshell, pdf-centric as all hell (or is supposed to be) which is exactly as intended

just my two cents

---

Astra: **Yes—I’d put these in one repository with a Cargo workspace, keep the npm packages independently publishable, and move XLSX ownership into `docdown`.** That gives you easier code reuse while keeping the package boundaries aligned with their purpose.

| Package | Responsibility |
|---|---|
| `pdfdown` | PDF structure, text, images, annotations, forms, and PDF-specific extraction |
| `docdown` | XLSX, DOCX, PPTX structure and embedded content; additional document formats later |
| `audiodown` | Audio parsing and extraction |
| `videodown` | Video/container parsing and extraction |

Your logos-and-charts observation strengthens the case for that move. Once XLSX support includes drawings, media, charts, tables, and formulas, it becomes a substantial document parser in its own right.

**A Cargo workspace fits the way these packages are developing.**

You get local path dependencies, coordinated changes across crates, workspace-wide checks, and shared dependency/build configuration. Cargo workspaces also share a lockfile and target directory. You can still build and test individual members. [The Cargo Book](https://doc.rust-lang.org/cargo/reference/workspaces.html?utm_source=chatgpt.com)

For your setup, I’d use:

- **Cargo workspace** for Rust crates and their dependencies.
- **pnpm workspace** for the npm packages, TypeScript tests, and release tooling.
- Your existing task runner, if useful, to coordinate those commands.

The practical win is being able to change a shared Rust component and all its consumers in one commit, test them together, and publish whichever packages need releases. Separate repositories can consume Git or registry dependencies, but they add coordination overhead during this kind of active development. [The Cargo Book](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html?utm_source=chatgpt.com)

**The important distinction is between reusable Rust libraries and the N-API entry points.**

Keep reusable code in ordinary Rust library crates, with the Node-facing adapters at the boundary. NAPI-RS builds the addon as a `cdylib`; its CLI supports selecting a Cargo manifest within a larger repository. [NAPI-RS](https://napi.rs/docs/introduction/simple-package?utm_source=chatgpt.com)

That matters here because the `pdfdown` manifest we reviewed currently declares only `cdylib`, and much of its implementation is `pub(crate)`. Moving directories into a monorepo alone won’t make those internals consumable by another crate. You need deliberate library boundaries.

I would extract shared crates when there is concrete reuse:

- Image inspection, decoding, or metadata utilities used by multiple packages.
- Audio/container primitives genuinely shared by `audiodown` and `videodown`.
- Small common asset or diagnostic types where their semantics actually match.

Within `docdown`, XLSX/DOCX/PPTX can initially remain modules sharing an internal OOXML implementation. Every module does not need to become a crate.

**Images should preserve both the asset and each occurrence of it.**

For ordinary embedded OOXML pictures, the useful distinction is:

| Representation | What it preserves |
|---|---|
| Image asset | Original encoded bytes, content type, intrinsic dimensions when available, source part |
| Image placement | Owning sheet/slide/document location, anchor, displayed extent, crop, rotation, descriptive metadata |

An XLSX drawing references image data through relationships, while its anchor describes placement. Spreadsheet anchors can use cell positions and offsets, with extents expressed in EMUs. [Microsoft Learn](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.blip?view=openxml-3.0.1\&utm_source=chatgpt.com)

That lets one logo asset have several placements without duplicating its buffer. It also prevents confusing a logo’s **pixel dimensions** with its **displayed size in the worksheet**.

Your existing image work is valuable here: reuse applicable inspection/codec code and the approach to preserving bytes and metadata. The XLSX relationship and drawing traversal belongs in `docdown`.

For XLSX, I’d make sheet identity and anchors authoritative. A PDF-style page number can be derived later if you introduce pagination.

**Charts absolutely belong in phase-I parsing—even if rendering waits.**

A native Excel chart can contain structured series, category/value references, cached data, axes, and drawing placement. It is not guaranteed to have a ready-made image that you can extract. Microsoft’s chart example builds those structures and attaches the chart through a drawing. [Microsoft Learn](https://learn.microsoft.com/en-us/office/open-xml/spreadsheet/how-to-insert-a-chart-into-a-spreadsheet?utm_source=chatgpt.com)

I’d have phase I preserve:

- Chart type, title, series names, axes, and legends.
- Series formulas/range references and cached values.
- Placement and relationships to its source data.
- Original chart XML and related unsupported content where structured coverage is incomplete.

Then chart rendering becomes a separate capability. You can already give downstream consumers meaningful chart data without implementing Excel’s rendering engine.

The same principle applies to text boxes, shapes, connectors, and grouped drawings: preserve their content and relationships even before you can reproduce their appearance.

**I’d make the repository move before significantly expanding the current XLSX implementation.** Establish the workspace, move spreadsheet ownership to `docdown`, then add the shared OOXML package/relationship/media machinery that DOCX and PPTX will also use. That avoids building more spreadsheet functionality around PDF-specific page and layout assumptions.

One terminology detail for the later phase: if you mean `.ods`, `.odt`, and `.odp`, those are **OpenDocument formats**. Google Sheets, Docs, and Slides can export to those formats or to OOXML; accessing the native Google documents is an API/export integration. I’d keep that acquisition adapter separate from the file parsers. [developers.google.com](https://developers.google.com/workspace/drive/api/guides/ref-export-formats?hl=en\&utm_source=chatgpt.com)

---

Andrew: if it's all in a monorepo anyway I might make repos by document type within -- eg, xlsxdown, docxdown, pptxdown, etc

---

Astra: **Yes—that’s a sensible split.** Separate workspace crates/packages for each document type, all in the same Git repository, would give you clearer ownership and let consumers install only what they need.

I’d organize the responsibilities roughly like this:

| Crate/package | Owns |
|---|---|
| `xlsxdown` | Workbooks, worksheets, cells, formulas, tables, spreadsheet drawings |
| `docxdown` | Document sections, paragraphs, runs, tables, headers/footers, document drawings |
| `pptxdown` | Presentations, slides, masters, layouts, notes, slide objects |
| `ooxml-core` | Package access, relationships, content types, XML handling, shared DrawingML/chart/media primitives |
| `docdown` | Format detection and dispatch to the appropriate parser |

Your existing `pdfdown`, `audiodown`, and `videodown` would remain peers in that workspace.

**`docdown` then becomes the convenient umbrella API.** Someone processing only spreadsheets could depend on `xlsxdown`; your broader ingestion pipeline could use `docdown`. Both would reach the same underlying implementation.

There are two independently useful boundaries here:

- **Rust crates:** establish implementation ownership and allow direct Rust reuse.
- **npm packages:** establish what JavaScript consumers install and which native binaries you distribute.

You can have separate format crates immediately and add independent N-API/npm packages wherever you want that public distribution boundary. Internal helpers such as `ooxml-core` don’t need their own npm package.

For the shared OOXML code, I’d keep the responsibilities concrete. Relationship resolution and embedded-media access are obvious shared pieces. Chart definitions and portions of DrawingML also cross formats. **Placement belongs with the host format:** an XLSX cell anchor, DOCX inline/floating anchor, and PPTX slide position have different semantics.

That also gives each parser room to return a faithful native model:

- `xlsxdown` → workbook/sheets/cells/formulas/tables.
- `docxdown` → sections/blocks/paragraphs/runs.
- `pptxdown` → slides/shapes/text frames/notes.

`docdown` can return a discriminated result containing whichever model applies. Text extraction, Markdown, and page-like layout can be derived views over those models.

The main cost is maintaining independent release and native-build targets if you publish every format separately. Given that you already maintain several N-API packages, that seems like a reasonable tradeoff—especially because **you now have actual format-specific complexity to justify the separation.**


---

Andrew: ooxmldown ;3 **

---

Astra: `ooxmldown` 😏

The shared Rust crate for package traversal, relationships, content types, XML, and common drawing/media structures—consumed directly by `xlsxdown`, `docxdown`, and `pptxdown`.

Then `docdown` is the umbrella entry point over those format-specific parsers. The naming practically documents the dependency structure.

