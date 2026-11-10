# Continuity — 2026-11-10 — Claude Fable 5.1

For the post-compaction self. Everything here is aggregate or invented; no private corpus value,
vendor name or private filename appears in this file (that rule is absolute for tracked files).

## 1. Where things stand (2026-11-10, early)

- **Repo** `~/rust/napi-rs` = `@d0paminedriven/pdfdown` (+ `ocr/` = `@d0paminedriven/pdfdown-ocr`).
  **Branch is `main`** (Andrew switched on 2026-11-09; `dev` is history). Never switch branches,
  never create worktrees; commit in place; Andrew pushes (the shell has no GitHub credentials).
- **Published:** 0.11.0 for both packages (2026-11-09). `main` carries several unpushed `chore`
  commits after that release (Node 26 `engines`, build-wrapper removal with CI calling
  `yarn napi build` directly, build script rename to `build:base`/`format:base` and
  `build:superset`/`format:superset`).
- **Andrew's uncommitted working tree (his, do not revert):** `scripts/post-build.ts` (prepends
  `@d0paminedriven/type-utils` re-exports and derived aliases `FormFieldType`,
  `FormFieldByType`, `FormFieldPayload` to `index.d.ts` / `ocr/index.d.ts`), `package.json`
  `modify:base` / `modify:superset` steps wired as `build:base && modify:base && format:base`,
  `types/dts-header.ts`, `type-utils.ts` deleted (replaced by the published package, now a
  runtime `dependency`), `ocr/package.json`, `pdfdown.wasi.d.cts`, `yarn.lock`, `.vscode`.
  `yarn typecheck` passes with the prepended block. Known nit: `modify:*` is not idempotent
  (prepends twice if run without a rebuild); Andrew rated it low priority.
- **Release flow:** CI publishes only when `main`'s head commit message is exactly the bare
  version (`0.11.0`); a PR merge commit never publishes; several same-version commits in a row
  are fine as long as nothing published yet. After bumping the version, run `yarn build &&
  yarn build:ocr` before committing so the generated loaders carry the version string
  (`preversion`/`version` scripts never run under Yarn 4; they are dead).

## 2. Work done in this session (chronological, condensed)

1. **Phase IV slice 1 (header fields from pages)** executed inline from `notes/2026-11-06/0711.md`,
   reviewed by a fresh reviewer, fix pass committed (4b59b5a). Pages addressed by index,
   lexicon overlap rule, title rows, parties, totals tiers, perf.
2. **Follow-ups from real documents:** column totals (amount under a `Total` header wins, else
   rightmost), cover-page summary over last-page job-card totals (page with a totals block
   first), Bill To from the last page, `$.00` amounts, aircraft-number rows, page roles
   (appended purchase order / delivery note / remittance pages set aside only when an invoice
   page remains; title must be large or bold — the first version withheld fields on ~600 boxed
   forms whose header cell said `PURCHASE ORDER`, caught by the private aggregates), label-
   language date-order prior (language-specific labels vote once per field; shared words such
   as `Total`/`Date` don't), date-shaped identifiers rejected, party name from the next column.
3. **Lexicon:** es/fr/de rows across every field; `Other` distractor rows (Ship Date, Previous
   Balance, Customer Project Manager …); **normalisation bug fixed** (lowercase before folding
   confusables; `TOTAL` vs `Total` had mismatched and only fuzzy-matched); `US$`/`R$`/… glyphs;
   a glyph alone in its column votes for the page currency.
4. **Private corpus tooling:** `__test__/private-corpus.spec.ts` writes `results/<id>.json`
   beside each PDF and an index `report.json`/`report.ts` with `kind`
   (native/scanned/mixed/empty), header fields read, rasters (codec, painted dpi) for scans.
   Corpus 2026-11-09: 6,949 PDFs = native 5,807 / scanned 983 / mixed 155 / empty 4; header
   track id 5,036 · date 5,079 · total 5,485 · vendor 5,921 · reconciled 3,918 · mean 0.80.
   Scans concentrate in the three oldest id-prefix groups (~32–34% each; recent groups 1–4%).
   Mixed documents are mostly a vendor scan plus the customer's own native PO page appended.
5. **OCR tuning:** design `notes/2026-11-07/2044.md` (builder-checked; two questions open for
   Andrew: labelling flow — my recommendation a local page with scan beside fields — and the
   JBIG2 route). Baseline on 321 scans of one group, label-free proxies, Tesseract default:
   id 58% / date 56% / total 65% / reconciled-to-the-cent 26%, 6 s/doc. **PSM sparse (11) is
   the knob:** 67% / 65% / 66% / 32%, 5 s/doc. Sauvola/adaptive thresholding neutral (bilevel
   fax rasters), upscaling to 300 dpi helps dates a little at +50% time, `PDFIUM_LIBRARY_PATH`
   did not engage the render tier on this machine. **JBIG2 is a decoding gap:** 163 of 1,138
   scanned+mixed documents carry JBIG2 streams, 79 have no decodable raster; candidate
   `hayro-jbig2` (MIT/Apache, same family as `hayro-jpeg2000`). The ocr env-var spike was
   discarded; `OcrOptions` (thresholding, minimumPpi, pageSegmentation) is the design's call.
6. **Azure Document Intelligence adapter:** `scripts/lib/azure-layout.ts` maps an Azure read
   result onto `PageLayout[]` (inches→points, y-up, text regions regrouped into baseline lines);
   `yarn invoice:from-azure <json>`. On a Spanish dot-matrix scan the reader gets id, date
   (day-first), PO, vendor, customer, total, USD, registration, serial from Azure's read while
   Tesseract's read of the same page is garbage: the OCR is the gap, not the reader. The incumbent
   pipeline itself uses Azure DI + Sonnet-generated per-vendor regex parsers (146 parser
   classes, 65,415 lines, inside an 89k-line extraction layer; first-seen formats go to
   "Needs Review"; the same PDF uploaded three times gave two "Needs Review").
7. **Engine direction:** PP-OCRv6 via `oar-ocr` (ort) behind an `OcrEngine` seam; Tesseract
   stays required until a dlopen shim makes it optional; bench decides defaults; bring-your-own
   batteries for public packages (runtime + models never bundled; private wrappers may
   bundle). surya is GPL/commercial-capped: reference only.
8. **CI/test policy (Andrew, 2026-11-09):** Node 26 only (`engines >= 26`); armv7 built but not
   tested (no `node:26` image for arm/v7); **no global ava timeout** (ava's default is 10 s
   inactivity; long tests carry `t.timeout()`); **CI slim, local rich**: the OCR CI step runs
   only `ocr/__test__/index.spec.ts` minus `*WithOcr*`/`*documentOcr*` plus `loader.spec.ts`
   (a synchronous Tesseract call blocks the ava worker so even `t.timeout()` cannot save it on
   a runner); `yarn test` = package specs only, `yarn test:scripts` = corpus tooling specs;
   Windows runners need LF fixtures (`.gitattributes` pins `*.xml` etc.) and POSIX `/` in row
   identifiers (`posixRelative` in `scripts/lib/corpus.ts`); the corpus directory is the fixed
   `ocr/__test__/fixtures/private` (environment variable removed); `native-tls`
   build-dependency removed (forced OpenSSL headers onto the cross toolchain); clippy 1.99 on
   runners (`some_filter`, `chunks_exact_to_as_chunks`); `mlugg/setup-zig@v2` Node-20 warning
   is harmless (no v3/v4 exists). `ava-config.d.ts` types the ava config files (value types
   only; JS object literals never flag unknown keys — verified, `@satisfies` and a generic
   helper both fail that test). `scripts/ava.config.mjs`, `ocr/ava.config.mjs` typed with it.
9. **Build scripts:** `build` = `build:base && modify:base && format:base` (Andrew's current
   tree); CI calls `yarn napi build --platform --release --no-const-enum --features …
   --target <triple>` directly (Yarn appends CLI args to the end of chained scripts, which is
   why the old `scripts/build.mjs` existed; it is deleted). Keep `--no-const-enum`: it makes
   napi emit string unions, which the typings and `IsExact` tests rely on. Yarn 4 runs no
   pre/post hooks for user scripts (verified; `enablePrePostScripts` is pnpm's).
10. **docdown / assetdown:** `~/rust/docdown/notes/brainstorm/2026-11-09.md` is revision 2: a
    general, context-agnostic document parser (native structure + common view of blocks with
    source refs; PDF and xlsx via pdfdown behind a facade; docx/pptx readers; conversion as a
    later superset with office2pdf first, LibreOffice optional via local `soffice` or a
    unoserver endpoint, Docker never spawned by the library). Astra's review corrections are in
    Appendix A. Andrew then created a Cargo workspace `~/rust/workspace` (repo `assetdown`,
    members `crates/xlsxdown` — manifest only so far) intended to host every `*down` crate
    (xlsxdown, docxdown, pptxdown, ooxmldown, pdfdown, videodown, audiodown). Advice given:
    core crates pure Rust with napi in a separate binding crate or behind a feature,
    `crate-type = ["cdylib","rlib"]` + `build.rs` for bindings, `napi build --manifest-path
    … --package-json-path …` per package, `ooxmldown` = port of pdfdown's bounded OOXML package
    reader (the `ooxml` crate on crates.io is two years stale), keep `reqwest` (rustls) out of
    parser crates, share lints/package metadata at workspace level.
11. **Access request** for read-only `AMS.Vendors` (vendor_id, vendor_number, vendor_name,
    country, status) in the QA2 database: drafted Description and Business Impact text; one
    daily bulk pull into a local JSON/TS lookup; purpose = resolve extracted vendor names to
    keys so the eval is complete (pipeline records' `vendorId` = `vendor_number`). A
    screenshot script for the ticket lives only in the session scratchpad (regenerate if
    needed: corpus rates, speed median 11.5 ms/doc p95 23 ms, and the one example document).
12. **Demo guidance** for the AI engineer meeting (snapshot zip on the work laptop; Tesseract
    5.5.0 + Leptonica 1.86 there, `capabilities()` → ocr true, render true): per-document
    `Promise.all` method lists (PDF: metadata/text/structuredText/layout/images/annotations/
    forms/attachments/document; xlsx: spreadsheetCells/layout/invoice only — PDF methods reject
    with `unsupported operation`; scans: textWithOcr/layoutWithOcr/documentOcr/renderPages).
13. **Workbook probe findings** (28 private workbooks): 7 of the 0.99-confidence results have a
    total of 0.00 "reconciled" with a subtotal of 0.00 → scoring flaw (zero total must not
    reconcile; warn + lower confidence); vendor heuristic accepts label text (`CONTRACT: …`,
    a column header word) → vendor candidates must not open with a label of any field.
    Both unfixed.

## 3. Docket (remaining, in Andrew's priority order as best known)

1. **Tests in sibling files** (Andrew's stated preference, 2026-11-10): move every inline
   `#[cfg(test)] mod tests { … }` into `<module>/tests.rs` declared as `#[cfg(test)] mod tests;`
   with `use super::*;` at the top. Module paths and all `super::…` references stay identical;
   private items remain visible. Start with `src/core/spreadsheet/format.rs`, then the rest of
   `spreadsheet/`, then `src/core/invoice/*` (one commit per directory). Verify with
   `cargo test --lib` and clippy `--all-targets`.
2. **Workbook reader fixes:** zero-total reconciliation (totals.rs `resolve`: a sum of zeros is
   not a reconciliation; zero invoice total → warning, no bonus); vendor candidates that open
   with a label or are a lone header word are rejected (parties.rs `vendor`).
3. **Engine serial ordinal:** `SERIAL MOTOR 1 P-100200` → leading one-digit ordinal before the
   value is skipped for identifier fields (layout.rs `best`).
4. **xlsx images:** parse `xl/drawings/drawingN.xml` anchors (EMU offsets) → `PlacedImage` on
   logical pages + an `imagesPerPage` shape for workbooks (2 of 28 private workbooks carry
   media; low priority).
5. **OCR bench plan** (writing-plans) once Andrew answers the labelling flow and the JBIG2
   route; JBIG2 decoding via `hayro-jbig2` is a likely prerequisite; `OcrOptions` knobs; the
   `OcrEngine` seam; PP-OCRv6 later as its own design.
6. **Expected files** from the incumbent pipeline's records → `scripts/lib/expected.ts` vocabulary
   (exclude system-assigned fields such as PO; `null` = absent, omitted = unknown; add
   `subTotal`/`totalTax` to `HEADER_FIELDS`); then `yarn invoice:eval` becomes the number.
7. **assetdown workspace:** `crates/xlsxdown` needs `src/lib.rs`, `[lib] crate-type`, `build.rs`;
   `ooxmldown` from pdfdown's `src/core/spreadsheet/package.rs`; decide core/binding split.
8. **Housekeeping:** trim napi targets to deployed platforms (Android, i686 Windows, FreeBSD,
   armv7 cost build minutes and are untested); pin `mlugg/setup-zig@v2.2.1`; delete dead
   `preversion`/`version` scripts; idempotency guard in `scripts/post-build.ts` (Andrew's).
9. **Phase IV slice 2:** line items, multi-page continuation, page roles for more kinds,
   address-country priors, `parties[]`, OCR-confusable folding in amounts.

## 4. Conventions and rules (verified this session)

- Formatting: oxfmt (`trailingComma: none`, semi, double quotes, sortImports), `cargo fmt`,
  taplo; type tests are bare `true satisfies IsExact<…>`; no `as` except `as const`.
- Dated notes: `notes/YYYY-MM-DD/HHMM.md`, H1 `# YYYY-MM-DD HH:MM — Title`.
- Never modify Andrew's files: `ocr/__test__/testing.ts`, his `ocr/__test__/private-corpus.spec.ts`,
  the private fetch script `ocr/__test__/fixtures/private/index.ts`, `scripts/post-build.ts`,
  `types/dts-header.ts`. pdfluent crates are reference only (AGPL). Nothing from
  the incumbent C# pipeline repo informs pdfdown's design (counting files/lines was fine).
- Private corpus values, filenames, vendor names never enter tracked files; aggregates only in
  logs/notes; the employer's name was scrubbed from the public README and fixtures (invented
  customer "Skyline Charter LLC").
- One general template-free engine, content-derived priors only; vendor knowledge post-
  extraction in a private TS package. Lookups cover the standard, never the sample.
- Commit trailer: `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Andrew's preferences voiced this session: async everywhere; `resolve(relative(process.cwd(),
  p))` for paths from outside; no environment variables where a fixed path will do; use
  `@d0paminedriven/fs` and `@d0paminedriven/type-utils` in TS tooling; CI slim, local rich;
  Node 26 only; "if it works it works" for scripts; pnpm everywhere outside napi repos.
- Global Constraints chain (local): `cargo fmt -- --check && cargo clippy --all-features
  --all-targets -- -D warnings && cargo test --all-features && yarn build && yarn build:ocr &&
  yarn test && yarn test:ocr --match '!private corpus*' && yarn test:scripts && yarn typecheck
  && yarn lint` (background when > 10 min; `yarn test` includes the private corpus, ~5 min).

## 5. Key files

- Invoice engine: `src/core/invoice/{lexicon,segments,anchors,parties,totals,priors,dates,
  amounts,identifiers,layout,testing,einvoice}.rs`; `src/types/invoice.rs`; seam
  `invoiceFromLayout` in `src/lib.rs`.
- Spreadsheet: `src/core/spreadsheet/{package,workbook,format,project,…}.rs`.
- OCR: `src/core/ocr.rs` (cascade, `only_pages`, `source_ppi`, `set_resolution`).
- Scripts: `scripts/lib/{corpus,profile,evaluate,expected,metrics,azure-layout,workbook}.ts`,
  `scripts/{corpus-profile,invoice-eval,invoice-from-azure}.ts`, `scripts/ava.config.mjs`.
- Specs: `__test__/*.spec.ts`, `__test__/private-corpus.spec.ts` (index + results),
  `ocr/__test__/{index,layout,loader}.spec.ts`, `scripts/__test__/*.spec.ts`.
- CI: `.github/workflows/CI.yml` (publish condition at the end; OCR test step filter).
- Memory (Claude's): `~/.claude/projects/-home-dopaminedriven-rust-napi-rs/memory/` —
  `pdfdown-forms-invoice-design.md`, `ci-and-test-policy.md`, `stay-on-dev.md` (now "main"),
  `invoice-architecture-decisions.md`, `sample-not-universe.md`, and the index `MEMORY.md`.
