# Port develop onto dev — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the work clone's phases II (attachments + e-invoice XML) and III (layout engine), CCITT decoding, rotated-page OCR, aircraft fields, `FormFieldPayload` and the private-corpus specs onto `dev` by cherry-pick, while keeping dev's hardened forms core, so `dev` becomes the single branch everything after this builds on.

**Architecture:** `dev` (this repo, `~/rust/napi-rs`) stays checked out throughout. The work clone's `develop` branch is fetched into a remote-tracking ref `work/develop` and ten of its fifteen commits are cherry-picked with `-x`, in dependency order, resolving conflicts by hand under one rule: dev's forms implementation always wins, additive hunks are kept from both sides, generated files are regenerated rather than resolved. Each group of picks ends with both packages built and every suite green before the next group starts. `develop` itself is never modified and stays in `~/rust/pdfdown` as a fallback until Andrew deletes it.

**Tech Stack:** git 2.34 (`cherry-pick -x`, `--trailer`), Rust 2024 / lopdf 0.46 / napi 3 / napi-derive 3.7, @napi-rs/cli 3.2, ava 8 + oxc-node, tsgo (`@typescript/native-preview`) for `yarn typecheck`, oxfmt / oxlint / cargo fmt / clippy / taplo, roxmltree 0.21 (phase II), hayro-ccitt 0.5 (CCITT).

**Spec:** The pick list in §0 below (agreed 2026-11-03), `notes/2026-11-02.md` §2–§3 (the divergence and why develop's features move onto dev), and the work clone's `notes/continuity/2026-11-02/overview.md` (what phases II/III contain). Design semantics for anything forms-related: `notes/2026-11-01.md` §5.

## 0. Pick list (the spec)

Develop commits since the common ancestor `3cf0429` (0.10.0), in order. Hashes are develop's and resolve once `work/develop` is fetched (Task 1).

| Commit    | Content                                                  | Decision                                                                                     |
| --------- | -------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `d3f2086` | develop's own forms core                                 | **skip** (dev's `forms.rs` is the hardened one; four of its unit tests are ported in Task 3) |
| `de996a2` | develop's forms fixtures, specs, type tests              | **skip** (one assertion and one test ported by hand in Task 6)                               |
| `c4549c5` | README/bench/CI for forms                                | **skip** (dev's `c950205` covers it)                                                         |
| `da72421` | CI on push to `main` only, drops PR trigger              | **skip** (dev→main PRs keep CI)                                                              |
| `a3052f4` | phase II: attachments + e-invoice XML                    | **take** (Task 4)                                                                            |
| `20c725e` | phase II tests, typecheck list, private path             | **take, adapt** (Task 5)                                                                     |
| `fc7a44d` | `UTR` type predicate in forms spec                       | **take by hand** (Task 6)                                                                    |
| `3481928` | `FormFieldPayload` header type + tests                   | **take** (Task 6)                                                                            |
| `3cec0b4` | aircraft references + work order fields                  | **take** (Task 8)                                                                            |
| `3ae19df` | docs for `AircraftReference`                             | **take** (Task 8)                                                                            |
| `92f9207` | private corpus report spec (`report.json` / `report.ts`) | **take** (Task 10)                                                                           |
| `5ed1960` | phase III layout engine                                  | **take** (Task 7)                                                                            |
| `74278bc` | CCITT decoding + rotated-page OCR                        | **take** (Task 9)                                                                            |
| `4a5a9b8` | continuity overview from work                            | **take, relocate** (Task 11)                                                                 |
| `8df4f62` | develop's four planning notes                            | **skip** (duplicates of dev's notes; remain in the clone)                                    |

## Global Constraints

- Stay on `dev`. No `git checkout`/`switch`, no worktrees, no stash of user files. The work clone `~/rust/pdfdown` is read-only for this plan.
- Every cherry-pick is `git cherry-pick -x <hash>`. A pick that conflicted is finished with `GIT_EDITOR=true git cherry-pick --continue` and then `git commit --amend --no-edit --trailer "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"`. Commits that are not cherry-picks end with the same trailer.
- Conflict rules: (1) `src/core/forms.rs`, `src/types/forms.rs`, `__test__/fixtures/forms/*`, `__test__/forms.spec.ts`, `__test__/forms.types.ts`, `ocr/__test__/forms.types.ts` are always resolved as **ours**; (2) additive hunks on both sides are **kept from both**; (3) `index.d.ts`, `index.js`, `ocr/index.d.ts`, `ocr/index.js`, `browser.js` are never hand-resolved: `git checkout --theirs -- <file>` then regenerate with `yarn build && yarn build:ocr` and stage the result.
- Formatting is oxfmt (`trailingComma: "none"`, `semi: true`, double quotes), `cargo fmt`, taplo for TOML. Prettier is not reintroduced. `trailingComma` is never `all`.
- No type assertions in TypeScript except `as const`; type-level tests are bare `true satisfies IsExact<…>` lines with no `void` statements (see Task 2 for the pattern).
- Private corpus path is `ocr/__test__/fixtures/private/` everywhere. No value, filename list, page text or vendor name from it enters any tracked file.
- Full verification suite, run at the end of Tasks 4–9 and in Task 12:
  ```bash
  cargo fmt -- --check && cargo clippy --all-features -- -D warnings
  cargo test && cargo test --all-features
  yarn build && yarn build:ocr
  yarn test && yarn test:ocr
  yarn typecheck && yarn lint
  ```
  Expected: every command exits 0; `cargo test` counts only grow; `git status --short` shows nothing under `ocr/__test__/fixtures/private/`.
- The OCR suite is slow on this machine because libtesseract oversubscribes OpenMP (see the work overview). Running it as `OMP_THREAD_LIMIT=1 yarn test:ocr` is allowed for speed; it changes no result. The package-level fix is a later task, not this plan.

## Review Focus

1. A hand-resolved `pub use types::{…}` list in `src/lib.rs` that drops a name from either side: `index.d.ts` silently loses a type. Pinned in Task 7 step 6 (the three `*.types.ts` files reference every public type and `yarn typecheck` must pass) and Task 12 step 2 (export-name diff against `work/develop`).
2. `TextSource` moving from a `cfg`-gated OCR type to an unconditional base export changes the base `index.d.ts`. Pinned by `__test__/layout.types.ts` (`IsExact<TextSource, "Native" | "Ocr" | "Rendered">`) arriving in Task 7.
3. A stale WASI loader after new exports: `pdfdown.wasi.cjs` would lack `layoutPerPage`. Pinned in Task 12 step 3.
4. The merged fixture generator producing different bytes for an existing forms fixture, breaking the qpdf cross-check. Pinned in Task 9 step 6 (regenerate, expect only the RC4 fixture to move, restore it).
5. The private-corpus specs on a machine without the directory. Both specs skip when `existsSync` fails; pinned in Task 10 step 4 by running the base spec with the directory temporarily renamed.

---

### Task 1: Prerequisites on dev

**Files:**

- Modify: `.gitattributes`
- Modify: `.gitignore`, `package.json`, `yarn.lock` (Andrew's pending edits, committed as they stand)
- Move: `ocr/__test__/private/` → `ocr/__test__/fixtures/private/` (untracked, gitignored)
- Modify: `ocr/__test__/fixtures/private/index.ts` (three path literals; untracked)

**Interfaces:**

- Produces: remote-tracking ref `work/develop` at `8df4f62`; `.gitattributes` without `merge=union`; private corpus at `ocr/__test__/fixtures/private/{345,353}/`.

- [ ] **Step 1: Confirm the starting state**

Run: `git status --short && git log -1 --format=%h`
Expected: `M .gitignore`, `M .vscode/settings.json`, `M ocr/__test__/forms.types.ts`, `M ocr/package.json`, `M package.json`, `M yarn.lock`, `?? .prettierrc`, `?? notes/CONTINUITY/README.md`, `?? notes/port-develop.md`; HEAD `e593531`.

- [ ] **Step 2: Remove the union merge driver**

In `.gitattributes`, change the six lines of the form `*.ts    text eol=lf merge=union` (for `*.ts`, `*.tsx`, `*.rs`, `*.js`, `*.json`, `*.debug`) to `*.ts    text eol=lf`. Leave every other line unchanged.

- [ ] **Step 3: Verify git no longer sees a merge driver**

Run: `git check-attr merge src/core/forms.rs __test__/forms.spec.ts package.json`
Expected: each line ends in `merge: unspecified`.

- [ ] **Step 4: Commit**

```bash
git add .gitattributes
git commit -m "chore(git): stop union-merging source files

merge=union concatenates both sides without conflict markers; a trial merge
of develop produced a 4,149-line forms.rs with duplicate functions.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

- [ ] **Step 5: Unify the private corpus path**

```bash
mkdir -p ocr/__test__/fixtures
mv ocr/__test__/private ocr/__test__/fixtures/private
sed -i 's#ocr/__test__/private/#ocr/__test__/fixtures/private/#g' ocr/__test__/fixtures/private/index.ts
sed -i 's#^ocr/__test__/private/\*$#ocr/__test__/fixtures/private/*#' .gitignore
```

- [ ] **Step 6: Verify the move and the ignore**

Run: `ls ocr/__test__/fixtures/private | tr '\n' ' '; echo; grep -c 'ocr/__test__/fixtures/private/' ocr/__test__/fixtures/private/index.ts; git status --short --ignored ocr/__test__/fixtures/private | head -2`
Expected: `345 353 index.ts real-ams-payload-post-processing.md shape.md`; `3`; one line starting `!! ocr/__test__/fixtures/private/`.

- [ ] **Step 7: Give the editor's Prettier extension a config that matches oxfmt**

The trailing commas come from the VS Code Prettier extension formatting on save with no project config (commit `1c74152` removed the `prettier` block from `package.json`; Prettier 3 defaults `trailingComma` to `all`). Prettier config keys are `trailingComma`, not `prettier.trailingComma`, so the current `.prettierrc` and the `prettier` block in `ocr/package.json` are ignored. Write `.prettierrc` as:

```json
{
  "arrowParens": "always",
  "bracketSameLine": true,
  "jsxSingleQuote": true,
  "printWidth": 120,
  "semi": true,
  "singleQuote": false,
  "tabWidth": 2,
  "trailingComma": "none",
  "useTabs": false
}
```

(every value shared with `.oxfmtrc.json` copied from it, so a save in the editor and `oxfmt --write` at commit time produce the same bytes). In `.vscode/settings.json` set `"prettier.arrowParens": "always"` to match; the other three `prettier.*` keys stay. Remove the `prettier` block from `ocr/package.json`: Prettier resolves config upward from the file, so the root `.prettierrc` already governs `ocr/`.

Run: `node -e 'const c=JSON.parse(require("fs").readFileSync(".prettierrc","utf8"));const ok=new Set(["arrowParens","bracketSameLine","jsxSingleQuote","printWidth","semi","singleQuote","tabWidth","trailingComma","useTabs"]);const bad=Object.keys(c).filter(k=>!ok.has(k));console.log(bad.length?"UNKNOWN "+bad:"ok", c.trailingComma)' && grep -c '"prettier"' ocr/package.json`
Expected: `ok none` and `0`.

- [ ] **Step 8: Commit the pending tooling edits with the ignore line and the editor config**

Run first: `yarn typecheck`
Expected: exit 0 (tsgo accepts `--ignoreConfig` and the two current files).

```bash
git add .gitignore package.json yarn.lock .prettierrc .vscode/settings.json ocr/package.json
git commit -m "chore: tsgo typecheck, @d0paminedriven/fs, private corpus path, prettier config mirroring oxfmt

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

Leave `ocr/__test__/forms.types.ts` uncommitted; Task 2 replaces it.

- [ ] **Step 9: Fetch develop as a remote-tracking ref**

Run: `git fetch /home/dopaminedriven/rust/pdfdown develop:refs/remotes/work/develop && git log -1 --format='%h %s' work/develop && git merge-base dev work/develop`
Expected: `8df4f62 docs(notes): add the pdfdown planning notes` and `3cf0429…`.

---

### Task 2: Void-free type tests for forms

**Files:**

- Modify: `types/dts-header.ts` (add `Equal`)
- Rewrite: `__test__/forms.types.ts`, `ocr/__test__/forms.types.ts`

**Interfaces:**

- Produces: `export type Equal<X, Y>` in both `index.d.ts` (Task 6 uses it); the house pattern for type-level tests (bare `true satisfies`, `@ts-expect-error` on `type` aliases, `import type` for functions used under `typeof`). Tasks 5, 6 and 8 add lines in this pattern.

- [ ] **Step 1: Add `Equal` to `types/dts-header.ts`, directly after `IsExact`, verbatim**

```ts
/**
 * Checks that X and Y are exactly equal.
 *
 * For instance, `Equal<'a', 'a'>` is true. But
 * `Equal<'a', 'b'>` is false.
 *
 * This also checks for exact intersection equality. So
 * `Equal<{ a: string; b: string  }, { a: string; b: string }>`
 * is true. But `Equal<{ a: string; b: string  }, { a: string; } & { b: string }>`
 * is false.
 */
export type Equal<X, Y> = (<T>() => T extends X ? 1 : 2) extends <T>() => T extends Y ? 1 : 2 ? true : false;
```

- [ ] **Step 2: Regenerate the typings**

Run: `yarn build && yarn build:ocr && grep -c 'export type Equal<X, Y>' index.d.ts ocr/index.d.ts`
Expected: `1` for each file.

- [ ] **Step 3: Replace both `forms.types.ts` files with this content (identical in both)**

```ts
import type { FormFieldByType, FormFieldType, IsExact, PageFormField, Rm, UnplacedFormField } from "../index";
import type { extractFormFieldsPerPage } from "../index";

// shape
true satisfies IsExact<
  FormFieldType,
  "Text" | "Checkbox" | "Radio" | "ComboBox" | "ListBox" | "PushButton" | "Signature" | "Unknown"
>;
true satisfies IsExact<keyof FormFieldByType, FormFieldType>;
true satisfies IsExact<FormFieldByType["Checkbox"], Rm<Extract<PageFormField, { type: "Checkbox" }>, "type">>;
true satisfies IsExact<UnplacedFormField["type"], FormFieldType>;
true satisfies IsExact<ReturnType<typeof extractFormFieldsPerPage>, PageFormField[]>;

// per-variant fields through the keyed lookup
true satisfies IsExact<FormFieldByType["Checkbox"]["checked"], boolean>;
true satisfies IsExact<FormFieldByType["Checkbox"]["onState"], string | undefined>;
true satisfies IsExact<FormFieldByType["ListBox"]["values"], string[] | undefined>;
true satisfies IsExact<FormFieldByType["Text"]["page"], number>;
true satisfies IsExact<FormFieldByType["Text"]["rect"], number[]>;

// negatives
// @ts-expect-error "type" is stripped from every keyed entry
type _NoDiscriminant = FormFieldByType["Text"]["type"];
// @ts-expect-error checked exists only on the Checkbox variant
type _NoCheckedOnText = FormFieldByType["Text"]["checked"];
```

- [ ] **Step 4: Verify it passes, and that it can fail**

Run: `yarn typecheck && yarn lint && npx oxfmt --check __test__/forms.types.ts ocr/__test__/forms.types.ts`
Expected: all exit 0.
Then temporarily change `["Checkbox"]["checked"], boolean` to `string` in one file, run `yarn typecheck`, expect `TS1360`, revert.

- [ ] **Step 5: Commit**

```bash
git add types/dts-header.ts index.d.ts ocr/index.d.ts index.js ocr/index.js browser.js __test__/forms.types.ts ocr/__test__/forms.types.ts
git commit -m "test(forms): type tests as bare satisfies assertions; Equal utility type in the header

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: Port develop's four forms unit tests

**Files:**

- Modify: `src/core/forms.rs` (test module only)

**Interfaces:**

- Consumes: develop's test module via `git show work/develop:src/core/forms.rs`: `widget_dict()` helper (lines 1166–1168), `inline_kid_gets_synthetic_field_object_id` (1337–1350), `radio_without_as_resolves_through_appearance_states_only` (1656–1691), `unplaced_signature_keeps_signed` (1881–1891), `inline_kid_dictionary_is_unplaced_under_the_logical_field_id` (1894–1911).

- [ ] **Step 1: Copy the helper and the four tests into dev's `#[cfg(test)] mod tests`**

Adapt only identifiers that differ between the two modules (fixture-building helpers, `FormsExtraction` field names). Do not change assertions yet.

- [ ] **Step 2: Run them**

Run: `cargo test forms::tests:: 2>&1 | grep -E '^test (forms::tests::(inline_kid|radio_without|unplaced_signature))|test result'`
Expected: four new test lines. For each failure apply this rule: if the assertion contradicts `notes/2026-11-01.md` §5 (dev's documented semantics: inline _field dictionaries_ get the synthetic `"<parent id>/kid I"` id, radios without `/AS` and without `/AP` select false with a warning unless the field has a single widget), rewrite the assertion to dev's semantics and keep the test; if it exposes behaviour §5 requires and dev lacks, fix `forms.rs` minimally until it passes.

- [ ] **Step 3: Full Rust suite**

Run: `cargo test 2>&1 | grep 'test result'`
Expected: `ok`, count ≥ 69 (65 + 4).

- [ ] **Step 4: Commit**

```bash
git add src/core/forms.rs
git commit -m "test(forms): port inline-kid, radio-without-AS and unplaced-signature cases from develop

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: Phase II — attachments and embedded e-invoice XML (`a3052f4`)

**Files:**

- Created by the pick: `src/core/attachments.rs`, `src/core/invoice/mod.rs`, `src/core/invoice/einvoice.rs`, `src/types/invoice.rs`
- Conflicts expected: `Cargo.toml` (+`roxmltree = "0.21.2"`), `src/core/mod.rs` (+2 `pub mod` lines), `src/types/mod.rs` (+2 lines), `src/lib.rs` (+118/−3: new functions, grown `pub use types::{…}` list), `README.md` (+167), `ocr/README.md` (+5/−1), generated `index.d.ts` / `ocr/index.d.ts`

**Interfaces:**

- Produces: `attachments(buffer)`, `attachmentsAsync`, `extractInvoice(buffer, options?)`, `extractInvoiceAsync`, `PdfDown.attachments[Async]`, `PdfDown.invoice[Async]`, types `Attachment`, `InvoiceResult`, `InvoiceFields`, `InvoiceItem`, `InvoiceOptions`, `MoneyField`, `StringField`, `DateField`, `NumberField`, `AddressField`, `TaxDetail`, `PaymentDetail`, `EmbeddedInvoiceXml`, `EInvoiceStandard`, `InvoiceSource`, `InvoiceDocumentType`, `DateOrder`, `FieldSpan`.

- [ ] **Step 1: Cherry-pick**

Run: `git cherry-pick -x a3052f4`
Expected: stops with conflicts in the files listed above (possibly fewer).

- [ ] **Step 2: Resolve**

`Cargo.toml`: keep dev's block, add the `roxmltree` line in alphabetical position. `src/core/mod.rs`, `src/types/mod.rs`: keep both sides' `mod` lines. `src/lib.rs`: one `pub use types::{…}` containing the union of both lists, sorted as cargo fmt leaves it; keep every function from both sides. `README.md`: insert develop's attachments and invoice sections after dev's "Utility types" section in API, and its usage examples after dev's "Combined: structured text" example. `ocr/README.md`: keep both. Generated files per the global rule.

- [ ] **Step 3: Format, build, regenerate**

Run: `cargo fmt && taplo fmt Cargo.toml && yarn build && yarn build:ocr && npx oxfmt README.md ocr/README.md`
Expected: both builds succeed; `git diff --stat index.d.ts ocr/index.d.ts` shows the new exports.

- [ ] **Step 4: Verify**

Run the full verification suite from Global Constraints.
Expected: green. `cargo test` count ≥ the Task 3 count plus develop's phase II unit tests.

- [ ] **Step 5: Finish the pick**

```bash
git add -A -- Cargo.toml src README.md ocr/README.md index.d.ts index.js ocr/index.d.ts ocr/index.js browser.js
GIT_EDITOR=true git cherry-pick --continue
git commit --amend --no-edit --trailer "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

Expected: `git log -1` shows develop's subject with `(cherry picked from commit a3052f4…)` and the trailer; `git status --short` is clean apart from the four uncommitted editor files.

---

### Task 5: Phase II tests and typecheck list (`20c725e`)

**Files:**

- Created by the pick: `__test__/invoice.spec.ts`, `__test__/invoice.types.ts`, `ocr/__test__/invoice.types.ts`, `__test__/private-corpus.spec.ts`, `__test__/fixtures/einvoice/*` (5 files)
- Conflicts expected: `.gitignore` (same line both sides), `package.json` (typecheck list), `examples/gen_fixtures.rs` (+157), `ocr/__test__/index.spec.ts` (+10), `__test__/forms.types.ts` and `ocr/__test__/forms.types.ts` (ours)

- [ ] **Step 1: Cherry-pick**

Run: `git cherry-pick -x 20c725e`

- [ ] **Step 2: Resolve**

`.gitignore`: one line `ocr/__test__/fixtures/private/*`. `package.json`: keep `tsgo` and the flags dev has; the file list becomes `__test__/forms.types.ts __test__/invoice.types.ts ocr/__test__/forms.types.ts ocr/__test__/invoice.types.ts`. `examples/gen_fixtures.rs`: keep both sides' generator functions and both sides' calls in `main`. `ocr/__test__/index.spec.ts`: keep both tests. Both `forms.types.ts`: `git checkout --ours`.

- [ ] **Step 3: Rewrite the two invoice type tests in the Task 2 pattern**

Replace the body of `__test__/invoice.types.ts` and `ocr/__test__/invoice.types.ts` (identical) with the eight shape assertions from develop as bare `true satisfies IsExact<…>` lines, plus:

```ts
true satisfies IsExact<ReturnType<typeof extractInvoice>, InvoiceResult>;
true satisfies IsExact<NonNullable<InvoiceResult["fields"]["invoiceTotal"]>["value"], number>;
true satisfies IsExact<NonNullable<InvoiceResult["fields"]["invoiceTotal"]>["text"], string>;
true satisfies IsExact<NonNullable<InvoiceResult["fields"]["vendorAddress"]>["lines"], string[]>;
true satisfies IsExact<NonNullable<InvoiceResult["fields"]["invoiceId"]>["confidence"], number>;
```

with `extractInvoice` under `import type`, no `declare const`, no `void`. If any `NonNullable<…>["lines"]` or `["text"]` is itself optional in `index.d.ts`, use the type `index.d.ts` states.

- [ ] **Step 4: Verify**

Run: `cargo run --example gen_fixtures && git status --short __test__/fixtures`
Expected: no changes (the committed e-invoice fixtures are bit-identical), or only `acroform-basic-rc4.pdf`, which is restored with `git checkout -- __test__/fixtures/forms/acroform-basic-rc4.pdf`.
Then the full verification suite. `yarn test` now runs `__test__/private-corpus.spec.ts` over the 336 local PDFs; it must pass and write nothing tracked.

- [ ] **Step 5: Finish the pick**

```bash
git add -A -- .gitignore package.json examples __test__ ocr/__test__
GIT_EDITOR=true git cherry-pick --continue
git commit --amend --no-edit --trailer "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: `FormFieldPayload` and the forms spec additions (`3481928`, `fc7a44d` by hand)

**Files:**

- Modify: `types/dts-header.ts` (+11 via pick), `README.md` (+1 via pick), `__test__/forms.types.ts`, `ocr/__test__/forms.types.ts`, `__test__/forms.spec.ts`

**Interfaces:**

- Produces: `export type FormFieldPayload<T extends keyof FormFieldByType = keyof FormFieldByType> = { [P in T]: FormFieldByType[P] }[T];` in both `index.d.ts`.

- [ ] **Step 1: Cherry-pick `3481928`, resolve both `forms.types.ts` as ours, keep README both**

Run: `git cherry-pick -x 3481928`, then `git checkout --ours -- __test__/forms.types.ts ocr/__test__/forms.types.ts`.

- [ ] **Step 2: Add the payload assertions to both `forms.types.ts`** (import `Equal` and `FormFieldPayload`; `Equal` is the stricter check, so payload identity uses it)

```ts
// FormFieldPayload: one key gives that variant's payload, a union of keys the union of payloads,
// no key is every payload, and "type" is never present
true satisfies Equal<FormFieldPayload<"Signature">, FormFieldByType["Signature"]>;
true satisfies Equal<FormFieldPayload<"Checkbox" | "Radio">, FormFieldByType["Checkbox"] | FormFieldByType["Radio"]>;
true satisfies Equal<FormFieldPayload, FormFieldByType[FormFieldType]>;
true satisfies Equal<FormFieldPayload<"Signature">["signed"], boolean>;
// @ts-expect-error "type" is stripped from every payload
type _NoTypeOnPayload = FormFieldPayload<"Text">["type"];
```

- [ ] **Step 3: Regenerate, verify, finish the pick**

Run: `yarn build && yarn build:ocr && yarn typecheck && yarn lint && npx oxfmt --check __test__/forms.types.ts ocr/__test__/forms.types.ts`
Expected: exit 0; `grep -c FormFieldPayload index.d.ts ocr/index.d.ts` prints `≥1` for each.

```bash
git add -A -- types __test__ ocr/__test__ README.md index.d.ts ocr/index.d.ts index.js ocr/index.js browser.js
GIT_EDITOR=true git cherry-pick --continue
git commit --amend --no-edit --trailer "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

- [ ] **Step 4: Port `fc7a44d` by hand — the `ofType` predicate**

Run: `grep -nE ' as (Extract|PageFormField|\{)' __test__/forms.spec.ts`
If it prints lines: add to `__test__/forms.spec.ts`

```ts
type FormFieldVariants = UTR<PageFormField, "type">;
const ofType =
  <T extends FormFieldType>(type: T) =>
  (f: PageFormField): f is FormFieldVariants[T] =>
    f.type === type;
```

(importing `FormFieldType`, `PageFormField`, `UTR` as types) and replace each printed assertion with `.filter(ofType("<Variant>"))` narrowing. If it prints nothing, there is nothing to port; record "fc7a44d: no assertions in dev's spec" in the ledger and skip.

- [ ] **Step 5: Steal two checks from develop's spec**

In the test that already asserts `inherited.unplacedFields` names, after `t.regex(inlinekid.fieldObjectId, /^\d+ \d+ obj$/);` add `t.false(/\/kid/.test(inlinekid.fieldObjectId));`. Add a new test:

```ts
test("real world: Sliced Invoices sample has no form at all", (t) => {
  const buf = readFileSync(
    join(import.meta.dirname, "fixtures", "invoices", "wordpress-pdf-invoice-plugin-sample.pdf")
  );
  t.is(pdfMetadata(buf).form, undefined);
  t.deepEqual(extractFormFieldsPerPage(buf), []);
});
```

- [ ] **Step 6: Verify and commit**

Run: `yarn test 2>&1 | tail -3 && yarn lint`
Expected: `yarn test` passes with the forms count up by 1.

```bash
git add __test__/forms.spec.ts
git commit -m "test(forms): ofType predicate, Sliced Invoices no-form case, inline kid keeps its own id

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: Phase III — layout engine (`5ed1960`)

**Files:**

- Created by the pick: `src/core/layout/{mod,interpreter,fonts,standard14,words}.rs`, `src/types/layout.rs`, `__test__/layout.spec.ts`, `__test__/layout.types.ts`, `ocr/__test__/layout.spec.ts`, `ocr/__test__/layout.types.ts`, `__test__/fixtures/layout/layout-sample.pdf`
- Conflicts expected: `src/types/mod.rs` (remove both `cfg`-gated `TextSource` enums, add `mod layout; pub use layout::*;`), `src/core/mod.rs` (+1), `src/core/meta.rs` (`get_inherited_page_box` becomes `pub(crate)`), `src/lib.rs` (+182/−4), `package.json` (typecheck list), `examples/gen_fixtures.rs` (+71), `README.md` (+76), `ocr/README.md` (+22/−4), generated files

**Interfaces:**

- Produces: `layoutPerPage(buffer)`, `layoutPerPageAsync`, `PdfDown.layout[Async]`; OCR package `extractLayoutWithOcrPerPage(buffer, options?)`, `extractLayoutWithOcrPerPageAsync`, `PdfDown.layoutWithOcr[Async]`; types `PageLayout`, `TextLine`, `TextWord`, `PlacedImage`; `TextSource` (`"Native" | "Ocr" | "Rendered"`) exported from the base package unconditionally.

- [ ] **Step 1: Cherry-pick**

Run: `git cherry-pick -x 5ed1960`

- [ ] **Step 2: Resolve**

`src/types/mod.rs`: delete dev's two `#[cfg(…)] pub enum TextSource` blocks, add develop's two lines. `src/core/meta.rs`: take develop's signature for `get_inherited_page_box`. `src/lib.rs`: union of `pub use types::{…}` (now including the layout types and `TextSource` from `types`), remove the OCR-only `pub use types::{OcrOptions, …, TextSource}` line only insofar as develop did, keep every function from both sides. `package.json`: file list adds `__test__/layout.types.ts ocr/__test__/layout.types.ts`, still `tsgo`. `examples/gen_fixtures.rs`, both READMEs: keep both. Generated files per the global rule.

- [ ] **Step 3: Format, build, regenerate**

Run: `cargo fmt && yarn build && yarn build:ocr && npx oxfmt README.md ocr/README.md package.json`

- [ ] **Step 4: Verify**

Run the full verification suite. Expected: green; `yarn test` now includes `__test__/layout.spec.ts`; `yarn test:ocr` includes `ocr/__test__/layout.spec.ts`.

- [ ] **Step 5: Fixture regeneration check**

Run: `cargo run --example gen_fixtures && git status --short __test__/fixtures`
Expected: nothing, or only `acroform-basic-rc4.pdf` (restore it).

- [ ] **Step 6: Export parity check against develop for this commit's surface**

Run: `for n in layoutPerPage layoutPerPageAsync PageLayout TextLine TextWord PlacedImage TextSource; do grep -q "\b$n\b" index.d.ts || echo "MISSING $n"; done; for n in extractLayoutWithOcrPerPage extractLayoutWithOcrPerPageAsync; do grep -q "\b$n\b" ocr/index.d.ts || echo "MISSING $n"; done`
Expected: no output.

- [ ] **Step 7: Finish the pick**

```bash
git add -A -- Cargo.toml src examples package.json README.md ocr/README.md __test__ ocr/__test__ index.d.ts index.js ocr/index.d.ts ocr/index.js browser.js
GIT_EDITOR=true git cherry-pick --continue
git commit --amend --no-edit --trailer "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: Aircraft references and work order (`3cec0b4`, `3ae19df`)

**Files:**

- Modify via picks: `src/types/invoice.rs`, `src/core/invoice/*`, `src/lib.rs` (+5/−5), `README.md` (+12), `__test__/invoice.types.ts`, `ocr/__test__/invoice.types.ts` (the `aircraft` / `engineSerialNumber` assertions, to be added in the Task 2 pattern)

- [ ] **Step 1: Cherry-pick both**

Run: `git cherry-pick -x 3cec0b4 && git cherry-pick -x 3ae19df`
If either stops: `src/lib.rs` keep develop's changed lines; `README.md` keep both; the two `invoice.types.ts` resolve as ours and then add

```ts
true satisfies IsExact<InvoiceResult["fields"]["aircraft"], AircraftReference[]>;
true satisfies IsExact<AircraftReference["engineSerialNumber"], StringField | undefined>;
```

- [ ] **Step 2: Verify**

Run: `yarn build && yarn build:ocr && cargo test && yarn test && yarn typecheck && yarn lint`
Expected: green; `grep -c AircraftReference index.d.ts` ≥ 1.

- [ ] **Step 3: Finish (only if a pick stopped)**

Same `--continue` and `--amend --trailer` sequence as Task 7 step 7.

---

### Task 9: CCITT decoding and rotated-page OCR (`74278bc`)

**Files:**

- Created by the pick: `__test__/fixtures/ccitt-g4-tiny.pdf`, `ocr/__test__/private-corpus.spec.ts`
- Conflicts expected: `Cargo.toml` (+`hayro-ccitt = "0.5.0"`), `__test__/index.spec.ts` (+17), `examples/gen_fixtures.rs` (+50), `README.md` (+1), `ocr/README.md` (+1)

**Interfaces:**

- Produces: `PageLayout.contentRotation: number` populated on OCR'd pages; CCITT Group 3/4 images decoded by image extraction and OCR tier 2.

- [ ] **Step 1: Cherry-pick and resolve (all keep-both; `Cargo.toml` adds the one dependency line)**

Run: `git cherry-pick -x 74278bc`

- [ ] **Step 2: Build and verify**

Run: `cargo fmt && taplo fmt Cargo.toml && yarn build && yarn build:ocr`, then the full suite with `OMP_THREAD_LIMIT=1 yarn test:ocr` in place of `yarn test:ocr`.
Expected: green. `ocr/__test__/private-corpus.spec.ts` OCRs only the scanned PDFs in the private directory (five here) and writes `report-ocr.json` there.

- [ ] **Step 3: Check the private directory stayed untracked**

Run: `git status --short --ignored ocr/__test__/fixtures/private | grep -vE '^!!' ; echo "tracked-or-untracked lines above: none expected"`
Expected: no lines before the echo.

- [ ] **Step 4: Fixture regeneration check**

Run: `cargo run --example gen_fixtures && git status --short __test__/fixtures`
Expected: nothing, or only `acroform-basic-rc4.pdf` (restore it).

- [ ] **Step 5: Finish the pick**

```bash
git add -A -- Cargo.toml src examples README.md ocr/README.md __test__ ocr/__test__ index.d.ts index.js ocr/index.d.ts ocr/index.js browser.js
GIT_EDITOR=true git cherry-pick --continue
git commit --amend --no-edit --trailer "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 10: Private corpus report (`92f9207`)

**Files:**

- Modify via pick: `__test__/private-corpus.spec.ts` (+73, no overlap with dev)

- [ ] **Step 1: Cherry-pick**

Run: `git cherry-pick -x 92f9207`
Expected: applies cleanly.

- [ ] **Step 2: Run the spec**

Run: `yarn test 2>&1 | grep -E 'private corpus|wrote|passed|failed'`
Expected: the private-corpus test passes and logs that it wrote `report.json` and `report.ts`.

- [ ] **Step 3: Check the outputs exist and are ignored**

Run: `ls ocr/__test__/fixtures/private/report.* && git status --short ocr/__test__/fixtures/private`
Expected: both files listed; `git status` prints nothing.

- [ ] **Step 4: Check the skip path**

Run: `mv ocr/__test__/fixtures/private ocr/__test__/fixtures/private.hold && yarn test 2>&1 | grep -E 'skipped|private corpus'; mv ocr/__test__/fixtures/private.hold ocr/__test__/fixtures/private`
Expected: the private-corpus test is reported as skipped, and the directory is back in place afterwards (`ls ocr/__test__/fixtures/private/345 | wc -l` prints the same count as before the rename). The rename is in place on the same filesystem, so nothing is copied, and `private.hold` must not exist when the step ends.

---

### Task 11: Notes and decisions

**Files:**

- Via pick then move: `notes/continuity/2026-11-02/overview.md` → `notes/CONTINUITY/2026-11-02/work-clone-overview.md`
- Modify: `notes/2026-11-02.md` §7
- Add: `notes/CONTINUITY/README.md` (currently untracked)

- [ ] **Step 1: Cherry-pick and relocate**

```bash
git cherry-pick -x 4a5a9b8
git mv notes/continuity/2026-11-02/overview.md notes/CONTINUITY/2026-11-02/work-clone-overview.md
rmdir -p notes/continuity/2026-11-02 2>/dev/null || true
git commit --amend --no-edit --trailer "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

- [ ] **Step 2: Record the decisions in `notes/2026-11-02.md` §7**

Append a subsection `### Decided 2026-11-03` with four lines: base branch is `dev`, develop's commits cherry-picked per `notes/port-develop.md` §0, develop retained in the work clone as a fallback until deleted; private corpus path is `ocr/__test__/fixtures/private/`; reference payload export for all corpus ids still pending; spreadsheet intake deferred, the invoice engine takes `PageLayout[]` so any positioned-text source can feed it later.

- [ ] **Step 3: Commit**

```bash
git add notes/2026-11-02.md notes/CONTINUITY/README.md
git commit -m "notes: 2026-11-03 decisions on base branch, corpus path and spreadsheet intake

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 12: Final verification

- [ ] **Step 1: Full suite**

Run the full verification suite from Global Constraints (with `OMP_THREAD_LIMIT=1` on the OCR suite).
Expected: all green. Record the counts: `cargo test` base and `--all-features`, ava base and ocr.

- [ ] **Step 2: Export parity against develop**

Run: `ex() { grep -hoE '^export (declare )?(function|class|interface|type|const enum|enum) [A-Za-z0-9_]+' "$1" | awk '{print $NF}' | sort -u; }; comm -13 <(ex index.d.ts) <(git show work/develop:index.d.ts | ex /dev/stdin); comm -13 <(ex ocr/index.d.ts) <(git show work/develop:ocr/index.d.ts | ex /dev/stdin)`
Expected: no output (every name develop exports, dev now exports).

- [ ] **Step 3: Regenerate the WASI loaders and typings (base package)**

Run: `yarn build --target wasm32-wasip1-threads && grep -c layoutPerPage pdfdown.wasi.cjs pdfdown.wasi-browser.js && git status --short '*.wasm'`
Expected: both counts ≥ 1; `git status` shows no `.wasm` (ignored). Commit the regenerated loaders and `.d.cts`:

```bash
git add pdfdown.wasi.cjs pdfdown.wasi-browser.js pdfdown.wasi.d.cts wasi-worker.mjs wasi-worker-browser.mjs 2>/dev/null
git commit -m "build(wasi): regenerate loaders for the layout, attachment and invoice exports

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>" || echo "nothing to commit"
```

If `ls ocr/*.wasi*` lists files, repeat for the OCR package with `--package-json-path ocr/package.json --output-dir ocr`; if the OCR WASI build fails on tesseract, record that in the ledger and leave the OCR loaders as they are.

- [ ] **Step 4: Working tree state**

Run: `git status --short`
Expected: nothing.

- [ ] **Step 5: Hand-off**

`dev` is now the single branch. Nothing is pushed by this plan; pushing `dev` and deleting `origin/develop` and the local `work/develop` ref are Andrew's calls, recorded in the ledger as open.
