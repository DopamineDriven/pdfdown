# Continuity — 2026-11-02 — anthropic / claude-fable-5-1

Written at ~1% context. Terse on purpose; nothing important omitted. Memory dir
(`~/.claude/projects/-home-dopaminedriven-rust-napi-rs/memory/`) holds the durable
preferences and decisions; read `MEMORY.md` first.

## Where we are (this clone: `~/rust/napi-rs`, branch `dev`, local only)

- `dev` HEAD `1a58717`, 43 commits ahead of stale `origin/dev` (0.9.9). Nothing pushed, nothing released.
- Done today, all on `dev`:
  - prettier → oxfmt + taplo (`.oxfmtrc.json`, `.prettierignore` deleted; TOML owned by taplo; lint-staged split).
  - Design `notes/2026-11-01.md` (forms read-only per page, layout, invoice; §5.1.1 structured-enum rules; §7.5 long invoices; §12 open decisions; §13 calls). Plan `notes/phase-1.md`. Reviews folded in (`notes/astra-feedback.md`).
  - **Phase 1 implemented + reviewed + fixed** (`2573019..bd93d6f`): `src/core/forms.rs`, `src/types/forms.rs` (macro-generated `PageFormField` structured enum, `FormInfo`, `UnplacedFormField`), `types/dts-header.ts` (+`napi.dtsHeaderFile` in both package.json), `extractFormFieldsPerPage[Async]`, `PdfDown.formFieldsPerPage[Async]`, `form?` on `PdfMeta`, `formFields/totalFormFields/formFieldPages` on `PdfDocument*`; fixtures `__test__/fixtures/forms/*` from `examples/gen_fixtures.rs`; ava `__test__/forms.spec.ts` (23), type tests `__test__/forms.types.ts` + `ocr/__test__/forms.types.ts`; README sections; CI adds `cargo test` + `yarn typecheck`; WASI loaders regenerated. Tests: cargo 65, ava 41, ocr ava 17, typecheck ok, clippy --all-features clean.
  - Review fixes (commit `bd93d6f`): self-referencing `/V` array no longer aborts process (`scalar_to_string`), duplicate `/Annots` widget deduped with warning, orphan widgets walk `/Parent`, dangling `/AcroForm` reports `FormInfo` + warning. Fixtures `acroform-hostile.pdf`, `acroform-dangling.pdf`.
  - `notes/2026-11-02.md`: corpus aggregates, two-clone divergence, reconciliation + POC plan. `__test__/fixtures/invoices/` (2 public vendor samples + README).
- Deferred minors (from review): unnamed kid under mixed-kids parent is `Unknown`; `/TU` treated inheritable; `.d.ts` keep napi single quotes (oxfmt skips .d.ts); `scan` helper shadowed in forms.spec.ts; napi build rewrites `browser.js` quotes (oxfmt restores).
- User's uncommitted edits present (do not revert): `@d0paminedriven/fs` dep, `.gitignore` `ocr/__test__/private/*`, prettier-style keys in `.prettierrc`/`ocr/package.json`/`.vscode/settings.json` (VS Code key names; prettier is removed — flagged to user).

## The other clone (`~/rust/pdfdown`, branch `develop`, pushed to `origin/develop`)

Phases I (own impl), II (attachments + e-invoice XML), III (layout: `src/core/layout/*`, `PageLayout.contentRotation`), CCITT decoding, rotated-page OCR retry, `InvoiceFields.aircraft[]`/`workOrder`, `InvoiceSource "None"`. Its `forms.rs` ≠ ours (~3k lines diff) and **lacks the four review fixes**. Continuity note there: `~/rust/pdfdown/notes/continuity/2026-11-02/overview.md` (OpenMP: libtesseract 8 s/page vs 0.6 s with `OMP_THREAD_LIMIT=1` — not fixed; Tesseract OSD pre-pass plan). Private corpus path there: `ocr/__test__/fixtures/private/`; here: `ocr/__test__/private/{345,353}/` (336 PDFs + 1 xlsx; 331 native / 5 scanned; producers iText 194, Qt 65; 197 five-page docs; one 111-page; 0 embedded XML; 3 AcroForms). Corpus values never go in the repo.

## Recommended next (see `notes/2026-11-02.md` §3, §7, §8)

1. Decision from Andrew: make `develop` the base, retire `dev` (recommended). Then port test-first: the 4 fixes (tests in this clone's `src/core/forms.rs`, generator cases), design doc + §7.5, plumbing (`.prettierignore` gone, oxfmt ignores, `*.wasm` ignore, CI steps), notes, `__test__/fixtures/invoices/`, unify corpus path.
2. Export Azure pipeline reference payloads per invoice id (`<id>.expected.json`, private dir); eval harness `scripts/invoice-eval.ts` per design §9/§6 of the 11-02 note; compare via a thin mapper to the downstream wire shape (`ocr/__test__/private/shape.md`, private).
3. OpenMP fix, then OSD; then phase IV plan on `develop`'s `PageLayout` (two dominant producer families first).

## Conventions (also in memory)

Stay on `dev` here (no worktrees/branches) — but the reconciliation will move work to `develop`; confirm with Andrew. Commits end with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. TS: no trailing commas, semicolons, double quotes (oxfmt). Dated notes `YYYY-MM-DD.md`. pdfdown is extraction-only (never fills forms). pdfluent = AGPL reference only, never copy. Tesseract-only OCR, no cloud. WASI: CI builds the .wasm but loaders/typings come from local `yarn build --target wasm32-wasip1-threads`, committed; `*.wasm` ignored. napi gotchas: macro `$ty` Option→null (use bracketed raw tokens), containers of output-only enums need `object_from_js = false`, `tsc` 7 needs `--ignoreConfig`. `cargo run --example` works on this cdylib crate. Superpowers skills were used (brainstorm → writing-plans → executing-plans Native → review); plan workspace `.superpowers/` is gitignored and was deleted after the clean review.
