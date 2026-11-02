# Phase 1: Per-page AcroForm Field Extraction — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add read-only, per-page AcroForm field extraction to `@d0paminedriven/pdfdown` and `@d0paminedriven/pdfdown-ocr`: `extractFormFieldsPerPage[Async]`, `PdfDown.formFieldsPerPage[Async]()`, `form?: FormInfo` on `PdfMeta` / `PdfDocument*`, and `formFields` / `totalFormFields` / `formFieldPages` on `pdfDocument*`, with `PageFormField` emitted as a TypeScript discriminated union.

**Architecture:** A new pure-lopdf module `src/core/forms.rs` walks `/AcroForm /Fields` into an internal arena, resolves inheritable attributes once per logical field, maps every page's `/Subtype /Widget` annotation to a placement, and emits one `PageFormField` per placed widget (plus `Unknown` entries for widgets no field claims, and `FormInfo.unplacedFields` for fields with no widget). `PageFormField` is a napi-rs 3.7 structured enum (native discriminated union, key `type`); the utility types (`Rm`, `UTR`, …) and derived aliases ride into both `index.d.ts` files through `napi.dtsHeaderFile`. Wiring follows the existing sync / `AsyncTask` / `PdfDown` pattern exactly.

**Tech Stack:** Rust 2024 (rustc 1.94), lopdf 0.46, napi 3 + napi-derive 3.7 (structured enums), @napi-rs/cli 3.2 (`dtsHeaderFile`), ava 8 + oxc-node for JS tests, qpdf 10 (fixture encryption + cross-check), oxfmt / cargo fmt / taplo.

**Spec:** `notes/2026-11-01.md` — §5 (Feature A, incl. 5.1.1 structured-enum rules), §9 (testing), §10 phase 1, §13 (calls). The spec travels with this plan; read §5 first.

## Global Constraints

- Read-only. No `/V` writes, no appearance streams, no flattening, no presentation data (`/DA`, `/Q`, `/MK`) surfaced. (spec §4, §5.1)
- Every `page` is a real 1-based page number. There is no `page: 0`. Fields without a placed widget go to `form.unplacedFields`. (spec §5.1, §13)
- API is 1:1 with the existing pattern: standalone sync fn + `*Async` fn on the libuv pool via `AsyncTask` + `PdfDown` methods sharing `Arc<Document>`; aggregates on `pdfDocument*` mirror `images` / `totalImages` / `imagePages`; document-level facts on `PdfMeta`. No new top-level `formInfo()` function. (spec §5.1, §8)
- `objectId` / `fieldObjectId` use the existing `"N G obj"` format from `PageImage.objectId` (`src/core/images.rs:176`); inline widget dicts get `"page P annot I"` (P 1-based, I 0-based). (spec §5.1)
- `PageFormField` is a `#[napi(object_from_js = false)]` structured enum with named-field variants only, no field named `type`, PascalCase literals, discriminator `type` (defaults). `FormFieldType` is derived in the `.d.ts` header as `PageFormField["type"]`, never a second Rust enum. (spec §5.1.1)
- `napi-derive = "3.7"` in `Cargo.toml` (lock already resolves 3.7.0). No new runtime dependencies for this phase. Everything must still compile for `wasm32-wasip1-threads` (pure Rust only). (spec §5.1.1, §7.4)
- Widget annotations keep appearing in `extractAnnotationsPerPage` with `subtype: "Widget"`; nothing existing changes shape. All existing ava tests keep passing. (spec §13)
- Sort order of the per-page array: `page` ascending, then top edge (`max(rect[1], rect[3])`) descending, then left edge (`min(rect[0], rect[2])`) ascending, then `name`, then `objectId`. `formFieldPages` = sorted unique pages. `totalFormFields === formFields.length === form.widgetCount`. (spec §5.1, §5.2)
- `FormInfo` is present when `/AcroForm` exists **or** any widget was found; absent otherwise. `fieldCount` = logical fields (placed + unplaced, excluding `Unknown`); `widgetCount` = placements. (spec §5.1)
- Nothing from pdfluent-sdk is copied: code, comments, fixtures. Implement from ISO 32000 + lopdf. (spec §3.1)
- TS/JS style: `trailingComma: none`, `semi: true`, double quotes (`.oxfmtrc.json`; lint-staged enforces on commit). Rust: `cargo fmt` defaults. TOML: taplo.
- Commit messages end with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.

## Review Focus

1. **Encrypted PDF with an empty user password** (RC4/AES, the common "no password but encrypted" case). Expected: same fields and values as the plaintext file; lopdf decrypts on load. Pinned in Task 6 (`encrypted` test) with a qpdf-encrypted copy of the basic fixture.
2. **Malformed `/Rect`** on a placed widget (3 numbers, non-numeric entries). Expected: the entry is still emitted with whatever numbers parsed (same raw convention as `PageAnnotation.rect`), sorting treats missing coordinates as 0, nothing panics. Pinned in Task 4 (unit) and Task 6 (`badrect` in the inherited fixture).
3. **Non-string `/V`** (integer, real, dictionary). Expected: numbers stringify (`42`), dictionaries/streams yield no `value` (signature dicts set `signed: true`). Pinned in Task 2 (unit) and Task 6 (`amount` field).
4. **Scale**: thousands of fields across many pages. Expected: linear behaviour, sorted output, counts agree. Pinned in Task 6 (`many` fixture: 1,500 fields, 30 pages).
5. **Non-ASCII field names in PDFDocEncoding** (`Stra\xDFe`), not UTF-16. Expected: decoded as `Straße` via `lopdf::decode_text_string`, never mojibake. Pinned in Task 2 (unit) and Task 6 (inherited fixture).

---

### Task 1: Types, structured enum, `.d.ts` header, build plumbing

**Files:**
- Modify: `Cargo.toml` (`napi-derive = "3.7"`)
- Move: `src/types.rs` → `src/types/mod.rs` (`git mv`, content unchanged, plus `mod forms; pub use forms::*;`)
- Create: `src/types/forms.rs`
- Create: `types/dts-header.ts`
- Modify: `package.json`, `ocr/package.json` (`napi.dtsHeaderFile`)
- Modify: `src/lib.rs:14-17` (`pub use types::{…}` gains `ChoiceOption, FormInfo, PageFormField, UnplacedFormField`)
- Test: `src/types/forms.rs` (`#[cfg(test)]`), plus build verification

**Interfaces:**
- Produces (Rust, `crate::types`):
  - `#[napi(object)] pub struct ChoiceOption { pub export_value: String, pub display_value: String }`
  - `#[napi(object)] pub struct UnplacedFormField { pub field_object_id: String, pub name: String, #[napi(js_name = "type", ts_type = "FormFieldType")] pub kind: String, pub value: Option<String> }`
  - `#[napi(object)] pub struct FormInfo { pub has_acro_form: bool, pub has_xfa: bool, pub need_appearances: bool, pub sig_flags: u32, pub field_count: u32, pub widget_count: u32, pub unplaced_fields: Vec<UnplacedFormField> }`
  - `pub(crate) struct FieldCommon { pub page: u32, pub rect: Vec<f64>, pub object_id: String, pub field_object_id: String, pub name: String, pub partial_name: String, pub widget_index: u32, pub widget_count: u32, pub value: Option<String>, pub default_value: Option<String>, pub tooltip: Option<String>, pub mapping_name: Option<String>, pub read_only: bool, pub required: bool, pub no_export: bool, pub has_actions: bool }`
  - `pub(crate) enum FieldKind { Text { multiline: bool, password: bool, comb: bool, rich_text: bool, max_len: Option<u32> }, Checkbox { on_state: String, checked: bool }, Radio { on_state: String, selected: bool }, ComboBox { options: Vec<ChoiceOption>, editable: bool }, ListBox { options: Vec<ChoiceOption>, multi_select: bool, values: Option<Vec<String>> }, PushButton {}, Signature { signed: bool }, Unknown {} }`
  - `#[napi(object_from_js = false)] pub enum PageFormField { Text {…}, Checkbox {…}, Radio {…}, ComboBox {…}, ListBox {…}, PushButton {…}, Signature {…}, Unknown {…} }` — every variant carries all `FieldCommon` fields plus its `FieldKind` fields.
  - `impl PageFormField { pub(crate) fn new(common: FieldCommon, kind: FieldKind) -> Self; pub(crate) fn page(&self) -> u32; pub(crate) fn rect(&self) -> &[f64]; pub(crate) fn name(&self) -> &str; pub(crate) fn object_id(&self) -> &str; pub(crate) fn kind_name(&self) -> &'static str }`
- Produces (TypeScript, prepended header): `Rm`, `Without`, `Include`, `XOR`, `CTR`, `RTC`, `IsExact`, `UTR`, `IsOptional`, `OnlyOptional`, `OnlyRequired` verbatim from `notes/types-ref.ts.md`, plus `export type FormFieldType = PageFormField["type"];` and `export type FormFieldByType = UTR<PageFormField, "type", true>;`

- [ ] **Step 1: Write the failing unit test** in `src/types/forms.rs`:

```rust
#[cfg(test)]
mod tests {
  use super::*;
  fn common() -> FieldCommon { /* page 2, rect [1.0,2.0,3.0,4.0], object_id "7 0 obj", field_object_id "5 0 obj", name "a.b", partial_name "b", widget_index 1, widget_count 2, value Some("v"), the rest None/false */ }
  #[test] fn new_copies_common_fields_into_every_variant() {
    let f = PageFormField::new(common(), FieldKind::Checkbox { on_state: "Ja".into(), checked: true });
    assert_eq!(f.page(), 2); assert_eq!(f.rect(), &[1.0, 2.0, 3.0, 4.0]); assert_eq!(f.name(), "a.b");
    assert_eq!(f.object_id(), "7 0 obj"); assert_eq!(f.kind_name(), "Checkbox");
    match f { PageFormField::Checkbox { on_state, checked, widget_count, .. } => { assert_eq!(on_state, "Ja"); assert!(checked); assert_eq!(widget_count, 2); } _ => panic!() }
  }
  #[test] fn unit_like_variants_still_carry_common_fields() {
    let f = PageFormField::new(common(), FieldKind::Unknown {});
    assert_eq!(f.kind_name(), "Unknown"); assert_eq!(f.page(), 2);
  }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --lib types::forms`
Expected: compile error, `PageFormField` not found.

- [ ] **Step 3: Bump napi-derive and move `types.rs`**

`Cargo.toml`: `napi-derive = "3.7"`. Then `git mv src/types.rs src/types/mod.rs` and append `mod forms;\npub use forms::*;` to it. `cargo check` must still pass before adding anything else.

- [ ] **Step 4: Implement `src/types/forms.rs`**

Write `ChoiceOption`, `UnplacedFormField`, `FormInfo`, `FieldCommon` by hand. Generate `PageFormField`, `FieldKind` and the `impl` from one `macro_rules!` so the common fields are declared once:

```rust
macro_rules! page_form_field {
  ($($variant:ident { $($field:ident : $ty:ty),* $(,)? }),+ $(,)?) => {
    #[napi(object_from_js = false)]
    pub enum PageFormField {
      $($variant { page: u32, rect: Vec<f64>, object_id: String, field_object_id: String,
        name: String, partial_name: String, widget_index: u32, widget_count: u32,
        value: Option<String>, default_value: Option<String>, tooltip: Option<String>,
        mapping_name: Option<String>, read_only: bool, required: bool, no_export: bool,
        has_actions: bool, $($field: $ty,)* },)+
    }
    pub(crate) enum FieldKind { $($variant { $($field: $ty,)* },)+ }
    impl PageFormField {
      pub(crate) fn new(c: FieldCommon, kind: FieldKind) -> Self {
        match kind { $(FieldKind::$variant { $($field,)* } => PageFormField::$variant {
          page: c.page, rect: c.rect, object_id: c.object_id, field_object_id: c.field_object_id,
          name: c.name, partial_name: c.partial_name, widget_index: c.widget_index,
          widget_count: c.widget_count, value: c.value, default_value: c.default_value,
          tooltip: c.tooltip, mapping_name: c.mapping_name, read_only: c.read_only,
          required: c.required, no_export: c.no_export, has_actions: c.has_actions, $($field,)* },)+ }
      }
      pub(crate) fn kind_name(&self) -> &'static str { match self { $(PageFormField::$variant { .. } => stringify!($variant),)+ } }
      // page(), rect(), name(), object_id(): one-arm-per-variant matches, same shape as kind_name()
    }
  };
}
page_form_field! {
  Text { multiline: bool, password: bool, comb: bool, rich_text: bool, max_len: Option<u32> },
  Checkbox { on_state: String, checked: bool },
  Radio { on_state: String, selected: bool },
  ComboBox { options: Vec<ChoiceOption>, editable: bool },
  ListBox { options: Vec<ChoiceOption>, multi_select: bool, values: Option<Vec<String>> },
  PushButton {},
  Signature { signed: bool },
  Unknown {}
}
```

Put `#[allow(dead_code)]` on `FieldCommon`, `FieldKind` and the `impl` block for now; Task 6 removes it. Fallback if napi-derive rejects the macro-generated enum: write the eight variants out longhand with the same fields (the TS output is identical).

- [ ] **Step 5: Run the unit tests**

Run: `cargo test --lib types::forms`
Expected: 2 passed.

- [ ] **Step 6: Create `types/dts-header.ts` and point both packages at it**

File starts with exactly `/* auto-generated by NAPI-RS */` and `/* eslint-disable */` on the first two lines (the header file replaces the CLI default), then the eleven utility types from `notes/types-ref.ts.md` and the two derived aliases. Add `"dtsHeaderFile": "types/dts-header.ts"` inside the `napi` object of `package.json` and of `ocr/package.json` (path is resolved from the repo root for both builds).

- [ ] **Step 7: Build and verify the generated typings**

Run: `yarn build:debug`
Then:
```bash
head -2 index.d.ts                                    # the two header comment lines
grep -n "export type PageFormField =" index.d.ts      # union alias present
grep -c "| { type: '" index.d.ts                      # expect 8 (napi-derive emits single quotes; lint-staged's oxfmt rewrites them to double on commit)
grep -n "FormFieldByType\|export type UTR" index.d.ts  # header content present
node_modules/.bin/tsc --noEmit --strict --types node index.d.ts   # header + generated types type-check together
node -e "require('./index.js'); console.log('addon loads')"
```
Expected: all greps hit, tsc exits 0, addon loads. If `PageFormField` is missing, the enum is not reachable from any `#[napi]` item yet — that is fine only if the grep still finds it; napi-derive emits typedefs for every `#[napi]` item regardless of use.

- [ ] **Step 8: Format, lint, commit**

```bash
cargo fmt && node_modules/.bin/oxfmt --check && cargo clippy --features render -- -D warnings
git add Cargo.toml Cargo.lock src/types src/lib.rs types/dts-header.ts package.json ocr/package.json index.d.ts index.js
git commit -m "feat(forms): PageFormField structured enum, FormInfo types, .d.ts utility header

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: Field tree walk, inheritance, string decoding

**Files:**
- Create: `src/core/forms.rs` (module declared `#[allow(dead_code)] pub(crate) mod forms;` in `src/core/mod.rs` until Task 6)
- Test: `src/core/forms.rs` `#[cfg(test)]` with an in-memory document builder

**Interfaces:**
- Consumes: `lopdf::{Document, Dictionary, Object, ObjectId}`, `lopdf::decode_text_string`, `crate::types::ChoiceOption`.
- Produces (all `pub(crate)` or private to the module):
  - `fn object_id_string(id: ObjectId) -> String` → `"N G obj"`
  - `fn decode_name_bytes(bytes: &[u8]) -> String` — UTF-8, else Latin-1 byte-for-byte
  - `fn decode_text_object(obj: &Object) -> Option<String>` — `String` → `decode_text_string` (lossy UTF-8 fallback on error); `Name` → `decode_name_bytes`; `Integer`/`Real` → `to_string()`; anything else → `None`
  - `struct FieldFlags(u32)` with `read_only() [bit 1]`, `required() [2]`, `no_export() [3]`, `multiline() [13]`, `password() [14]`, `no_toggle_to_off() [15]`, `radio() [16]`, `push_button() [17]`, `combo() [18]`, `edit() [19]`, `multi_select() [22]`, `comb() [25]`, `rich_text() [26]` — bit N means `1 << (N - 1)`
  - `enum FieldType { Text, Button, Choice, Signature }` from `/FT` `Tx | Btn | Ch | Sig`
  - `struct FieldNode { object_id: Option<ObjectId>, partial_name: String, field_type: Option<FieldType>, flags: Option<u32>, value: Option<Object>, default_value: Option<Object>, max_len: Option<u32>, options: Option<Vec<ChoiceOption>>, tooltip: Option<String>, mapping_name: Option<String>, rect: Option<Vec<f64>>, appearance_state: Option<String>, has_actions: bool, parent: Option<usize>, children: Vec<usize> }`
  - `struct FieldTree { nodes: Vec<FieldNode> }` with `fn walk(doc: &Document, fields: &[Object]) -> FieldTree` (visited `HashSet<ObjectId>`, depth cap 100, `/Kids` entries that are not dictionaries skipped), `fn effective_type(&self, i: usize) -> Option<FieldType>`, `fn effective_flags(&self, i) -> FieldFlags`, `fn effective_value(&self, i) -> Option<&Object>`, `fn effective_default(&self, i) -> Option<&Object>`, `fn effective_max_len(&self, i) -> Option<u32>`, `fn effective_options(&self, i) -> Option<&[ChoiceOption]>`, `fn effective_tooltip(&self, i) -> Option<&str>`, `fn full_name(&self, i) -> String` (non-empty partial names joined with `.`), `fn is_logical(&self, i) -> bool` (has effective type and no child with a non-empty partial name)
  - `fn parse_options(doc: &Document, obj: &Object) -> Vec<ChoiceOption>` — string entries → export = display; `[export display]` pairs; references resolved
  - `fn parse_rect(obj: &Object) -> Vec<f64>` — every numeric element in file order, non-numeric skipped (same as `collect_page_annotations` in `src/core/document.rs`)

- [ ] **Step 1: Write the failing unit tests** (one `mod tests` with a helper `fn doc(objects: Vec<(ObjectId, Object)>) -> Document` that inserts objects into `Document::with_version("1.7")`):

```rust
#[test] fn flags_use_one_based_bit_numbers() { let f = FieldFlags(1 << 12 | 1 << 24 | 1); assert!(f.multiline() && f.comb() && f.read_only()); assert!(!f.radio()); }
#[test] fn object_id_matches_page_image_format() { assert_eq!(object_id_string((12, 0)), "12 0 obj"); }
#[test] fn name_bytes_utf8_then_latin1() { assert_eq!(decode_name_bytes("Ja".as_bytes()), "Ja"); assert_eq!(decode_name_bytes(&[0xF6]), "ö"); }
#[test] fn text_object_decodes_utf16_pdfdoc_and_numbers() {
  assert_eq!(decode_text_object(&Object::String(vec![0xFE,0xFF,0x00,0x4D,0x00,0xFC], StringFormat::Hexadecimal)), Some("Mü".into()));
  assert_eq!(decode_text_object(&Object::String(b"Stra\xDFe".to_vec(), StringFormat::Literal)), Some("Straße".into()));
  assert_eq!(decode_text_object(&Object::Integer(42)), Some("42".into()));
  assert_eq!(decode_text_object(&Object::Dictionary(Dictionary::new())), None);
}
#[test] fn inheritance_resolves_type_flags_value_through_parent() { /* parent: FT Tx, Ff 1, V (shared); child: T "kid" only → effective_type Text, read_only, value "shared", full_name "parent.kid" */ }
#[test] fn container_with_named_child_is_not_logical_but_child_is() { /* parent FT Tx, child T "kid" with Rect → is_logical(parent)=false, is_logical(child)=true */ }
#[test] fn unnamed_kids_make_parent_logical() { /* radio group with 2 unnamed kids → is_logical(group)=true, is_logical(kid)=false */ }
#[test] fn kids_cycle_terminates_and_dedupes() { /* A /Kids [B], B /Kids [A] → walk returns 2 nodes, B.children empty */ }
#[test] fn depth_is_capped() { /* chain of 150 nested kids → nodes.len() <= 100 */ }
#[test] fn options_accept_strings_and_pairs() { /* [(a) [(US)(United States)]] → [{a,a},{US,United States}] */ }
#[test] fn rect_keeps_numeric_entries_in_order() { assert_eq!(parse_rect(&Object::Array(vec![Object::Integer(0), Object::Name(b"x".to_vec()), Object::Real(100.5)])), vec![0.0, 100.5]); }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib core::forms`
Expected: compile errors (module missing).

- [ ] **Step 3: Implement the module** with the signatures in Interfaces. `effective_*` walk `parent` links upward and return the first `Some`. `walk` pushes a node before recursing into `/Kids`, resolving `Object::Reference` kids through `doc.get_dictionary` and recording `object_id` only for referenced dictionaries.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib core::forms`
Expected: 11 passed.

- [ ] **Step 5: Format, lint, commit**

```bash
cargo fmt && cargo clippy --features render -- -D warnings
git add src/core/mod.rs src/core/forms.rs
git commit -m "feat(forms): AcroForm field tree walk with inheritance and text decoding

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: Widget scan and field-kind resolution

**Files:**
- Modify: `src/core/forms.rs`
- Test: same file

**Interfaces:**
- Consumes: Task 2's `FieldTree`, `FieldFlags`, `FieldType`, `decode_*`, `parse_rect`, `object_id_string`.
- Produces:
  - `struct PlacedWidget { object_id: String, key: Option<ObjectId>, page: u32, rect: Vec<f64>, dict: Dictionary }` — `key` is `None` for an inline dictionary, whose `object_id` is `format!("page {page} annot {index}")`
  - `fn scan_widgets(doc: &Document) -> Vec<PlacedWidget>` — iterates `doc.get_pages()`; resolves `/Annots` whether direct array or reference (mirror lopdf's `annots_container` logic; do not call `get_page_annotations`, it drops object ids); keeps only `/Subtype /Widget`
  - `fn on_state_of(dict: &Dictionary, doc: &Document) -> Option<String>` — first key of `/AP /N` (resolving references) that is not `Off`, decoded with `decode_name_bytes`
  - `fn radio_export_name(on_state: &str, options: Option<&[ChoiceOption]>) -> String` — when `options` is present and `on_state` parses as an index in range, return `options[idx].export_value`; else `on_state`
  - `fn resolve_kind(tree: &FieldTree, i: usize, widget: Option<&PlacedWidget>, doc: &Document) -> FieldKind` with the rules below
  - `fn common_for(tree: &FieldTree, i: usize, widget: &PlacedWidget, widget_index: u32, widget_count: u32, doc: &Document) -> FieldCommon`

Kind rules (`FieldType` → `FieldKind`):
- `Text` → `Text { multiline, password, comb, rich_text, max_len }` from effective flags / `/MaxLen`.
- `Button` with `push_button()` → `PushButton {}`; with `radio()` → `Radio { on_state: radio_export_name(widget on-state or "Yes", effective_options), selected }` where `selected` = widget `/AS` is non-`Off` **or** (no `/AS` and group `/V` name equals this widget's raw on-state); else `Checkbox { on_state, checked }` with the same `/AS`-then-`/V` rule.
- `Choice` with `combo()` → `ComboBox { options, editable: edit() }`; else `ListBox { options, multi_select, values }` where `values` = `Some(all strings)` when `/V` is an array, else `None`.
- `Signature` → `Signature { signed: effective_value is a Dictionary or Reference }`.
- `common_for.value`: Text/Choice → `decode_text_object(/V)` (array → first element); Button → the group's `/V` name passed through `radio_export_name` (so `/V /1` with `/Opt` becomes `Green`); Signature → `None`. `default_value` from `/DV` the same way. `has_actions` = node or widget dict has `/AA` or `/A`. `field_object_id` = `object_id_string(node.object_id)` or, for an inline field dict, the widget's `object_id`.

- [ ] **Step 1: Write the failing unit tests**

```rust
#[test] fn scan_finds_widgets_behind_indirect_annots_and_inline_dicts() { /* page1 /Annots → ref to array [ref widget]; page2 /Annots [inline widget dict]; non-widget /Link annot ignored → 2 widgets, pages [1,2], second.object_id == "page 2 annot 0", second.key.is_none() */ }
#[test] fn on_state_is_first_non_off_ap_key() { /* /AP << /N << /Off 1 0 R /Ja 2 0 R >> >> → Some("Ja"); no /AP → None */ }
#[test] fn radio_export_name_maps_indices_through_opt() { let o = [ChoiceOption{export_value:"Red".into(),display_value:"Red".into()}, ChoiceOption{export_value:"Green".into(),display_value:"Green".into()}]; assert_eq!(radio_export_name("1", Some(&o)), "Green"); assert_eq!(radio_export_name("Blue", Some(&o)), "Blue"); assert_eq!(radio_export_name("1", None), "1"); }
#[test] fn checkbox_checked_from_as_then_v() { /* AS /Ja → checked; no AS, V /Ja → checked; AS /Off, V /Ja → unchecked */ }
#[test] fn radio_selected_only_on_matching_widget() { /* group V /1, kids AS /Off,/1,/Off → selected [false,true,false], value Some("Green") with Opt */ }
#[test] fn choice_kinds_and_values() { /* combo+edit → ComboBox editable; list multiselect V [(a)(c)] → ListBox values Some([a,c]), common value Some("a") */ }
#[test] fn signature_signed_when_v_present() { /* V 9 0 R → Signature{signed:true}; no V → false; common value None */ }
#[test] fn non_string_values_and_actions() { /* V 42 → Some("42"); V << >> → None; widget with /A → has_actions */ }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib core::forms`
Expected: compile errors for the new names.

- [ ] **Step 3: Implement `scan_widgets`, `on_state_of`, `radio_export_name`, `resolve_kind`, `common_for`** per the rules above.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib core::forms`
Expected: 19 passed (11 from Task 2 + 8).

- [ ] **Step 5: Format, lint, commit**

```bash
cargo fmt && cargo clippy --features render -- -D warnings
git add src/core/forms.rs
git commit -m "feat(forms): widget page scan and per-kind field resolution

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: Emission, sorting, `FormInfo`, `extract_forms`

**Files:**
- Modify: `src/core/forms.rs`
- Test: same file

**Interfaces:**
- Consumes: Tasks 2–3.
- Produces:
  - `pub(crate) struct FormsExtraction { pub info: Option<FormInfo>, pub fields: Vec<PageFormField> }`
  - `pub(crate) fn extract_forms(doc: &Document) -> FormsExtraction` — the only entry point later tasks call
  - `pub(crate) fn form_field_pages(fields: &[PageFormField]) -> Vec<u32>` — sorted unique `page()` values
  - `fn sort_fields(fields: &mut [PageFormField])` — comparator from Global Constraints; a missing coordinate counts as `0.0`

Algorithm for `extract_forms` (spec §5.2):
1. `widgets = scan_widgets(doc)`; index them by `key` (`HashMap<ObjectId, usize>`).
2. `acroform = catalog /AcroForm` via `Dictionary::get_deref`. Read `/NeedAppearances` (bool, default false), `/SigFlags` (int, default 0), `/XFA` presence. Missing `/AcroForm` → `has_acro_form = false`, empty tree, **continue**.
3. `tree = FieldTree::walk(doc, /Fields)`. For each logical node: its placements are the node itself (if its `object_id` is in the widget index) or its children whose `object_id` is in the index. `widget_count` = number of placements. Emit `PageFormField::new(common_for(...), resolve_kind(...))` per placement and mark the widget claimed. A logical node with zero placements → `UnplacedFormField { field_object_id, name: full_name, kind: kind_name of resolve_kind(.., None, ..), value }`,
   with `value` computed exactly as `common_for` computes it (no widget involved).
4. Every unclaimed widget → `PageFormField::new(common from the widget dict alone: name = decoded /T or "", value = decode_text_object(/V), flags from /Ff if present, field_object_id = object_id, widget_index 0, widget_count 1, …), FieldKind::Unknown {})`.
5. `sort_fields`. `info = Some(FormInfo { has_acro_form, has_xfa, need_appearances, sig_flags, field_count: logical nodes (placed + unplaced), widget_count: fields.len(), unplaced_fields })` when `acroform.is_some() || !fields.is_empty()`, else `None`.

- [ ] **Step 1: Write the failing unit tests**

```rust
#[test] fn no_acroform_no_widgets_gives_none_and_empty() { /* info None, fields empty */ }
#[test] fn widgets_without_acroform_become_unknown_with_info() { /* 2 orphans → 2 Unknown, info.has_acro_form false, field_count 0, widget_count 2, names from /T */ }
#[test] fn unplaced_field_goes_to_info_not_array() { /* Tx field, no Rect, V "secret" → fields empty, unplaced_fields == [{name:"hidden", kind:"Text", value:"secret"}], field_count 1, widget_count 0 */ }
#[test] fn multi_page_field_emits_one_entry_per_placement() { /* 2 unnamed kids on pages 2,3 → widget_index [0,1], widget_count 2, same field_object_id, form_field_pages [2,3] */ }
#[test] fn duplicate_names_keep_distinct_field_object_ids() { /* two fields T "dup" → field_object_id differ */ }
#[test] fn sort_is_page_then_top_down_then_left_right_then_name() { /* craft 5 entries incl. one with rect len 3 → expected order */ }
#[test] fn field_count_excludes_unknown_and_counts_unplaced() { /* 1 placed logical + 1 unplaced + 1 orphan → field_count 2, widget_count 2 */ }
#[test] fn acroform_dictionary_flags_are_reported() { /* /NeedAppearances true /SigFlags 3 /XFA [] → info.need_appearances, sig_flags 3, has_xfa true; without them → false, 0, false */ }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib core::forms`
Expected: compile errors for `extract_forms` / `FormsExtraction`.

- [ ] **Step 3: Implement `extract_forms`, `form_field_pages`, `sort_fields`.**

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib core::forms`
Expected: 27 passed.

- [ ] **Step 5: Format, lint, commit**

```bash
cargo fmt && cargo clippy --features render -- -D warnings
git add src/core/forms.rs
git commit -m "feat(forms): emit per-page form fields, FormInfo, deterministic ordering

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: Fixture generator and committed fixtures

**Files:**
- Create: `examples/gen_fixtures.rs` (lopdf only; `cargo run --example gen_fixtures` works against the cdylib-only crate, verified)
- Create (generated, committed): `__test__/fixtures/forms/acroform-basic.pdf`, `acroform-inherited.pdf`, `acroform-cycle.pdf`, `acroform-orphans.pdf`, `acroform-many.pdf`, `acroform-basic-encrypted.pdf`

**Interfaces:**
- Produces: the six fixtures with the exact contents below. Tests in Task 6 assert these numbers; change both together or neither.

Shared conventions: pages are `MediaBox [0 0 612 792]`; every widget has `/Type /Annot /Subtype /Widget /F 4`; appearance streams are empty `/Subtype /Form /BBox [0 0 20 20]` streams; `/AcroForm /DA (/Helv 0 Tf 0 g)`.

`acroform-basic.pdf` — 1 page, `/NeedAppearances true`, `/Fields` with 9 entries (11 widgets), listed with `/Rect`:
| field | `/FT` + flags | value etc. | rect |
|---|---|---|---|
| `name` | Tx | `/V (Jane Doe) /TU (Full name) /TM (fullName)` | `[50 700 300 720]` |
| `notes` | Tx, Ff Multiline (1<<12) | `/MaxLen 200`, `/V` = `text_string("Line one\nLine two")` | `[50 600 500 680]` |
| `zip` | Tx, Ff Comb (1<<24) | `/MaxLen 5 /V (60601)` | `[50 560 150 580]` |
| `agree` | Btn, Ff Required (1<<1) | `/V /Ja /AS /Ja /AP << /N << /Ja s /Off s >> >>` | `[50 520 70 540]` |
| `color` | Btn, Ff Radio (1<<15) \| NoToggleToOff (1<<14) | `/Opt [(Red)(Green)(Blue)] /V /1`; 3 unnamed kids with `/AP /N` keys `/0`, `/1`, `/2`, `/AS` `/Off`, `/1`, `/Off`, `/Parent` back-ref | `[50 480 70 500]`, `[100 480 120 500]`, `[150 480 170 500]` |
| `country` | Ch, Ff Combo (1<<17) \| Edit (1<<18) | `/Opt [[(US)(United States)] [(DE)(Germany)]] /V (US)` | `[50 440 250 460]` |
| `tags` | Ch, Ff MultiSelect (1<<21) | `/Opt [(a)(b)(c)] /V [(a)(c)] /I [0 2]` | `[50 340 250 420]` |
| `submit` | Btn, Ff Pushbutton (1<<16) | `/A << /S /ResetForm >>` | `[50 300 150 320]` |
| `sig` | Sig | no `/V` | `[300 300 500 340]` |
Expected order (top edge desc, then left): `name, notes, zip, agree, color×3, country, tags, sig, submit`.

`acroform-inherited.pdf` — 3 pages; page 2's `/Annots` is an **indirect reference** to its array:
| field | structure | page / rect |
|---|---|---|
| `person` | `/FT /Tx /Ff 1` on the parent; kids `first` (`/V (Ada)`) and `last` (`/V` = `text_string("Müller")`, UTF-16BE) | 1 / `[50 700 200 720]`, `[250 700 400 720]` |
| `id` | Tx `/V (A-1)`, two **unnamed** widget kids | 2 / `[50 700 200 720]`; 3 / `[50 700 200 720]` |
| `hidden` | Tx `/V (secret)`, no `/Rect`, no kids | unplaced |
| `dup` ×2 | two distinct objects, both `/T (dup)`, `/V (one)` / `/V (two)` | 1 / `[50 600 200 620]`, `[50 560 200 580]` |
| `Straße` | `/T` as raw PDFDocEncoding bytes `53 74 72 61 DF 65`, `/V (x)` | 1 / `[50 500 200 520]` |
| `amount` | Tx `/V 42` (Integer) | 1 / `[50 460 200 480]` |
| `badrect` | Tx `/V (b)` | 1 / `/Rect [0 0 100]` |
Expected: 9 placements (page 1: 7, page 2: 1, page 3: 1), `fieldCount` 9, `unplacedFields` = `[hidden]`, `formFieldPages` `[1, 2, 3]`.

`acroform-cycle.pdf` — 1 page; `/Fields [A]`; `A`: `/T (A) /FT /Tx /Kids [B]`; `B`: `/T (B) /Kids [A] /Rect [50 700 200 720]`, B in page `/Annots`. Expected: exactly 1 placement named `A.B`, `fieldCount` 1.

`acroform-orphans.pdf` — 2 pages, **no** `/AcroForm`. Page 1 `/Annots [ref]` → widget `/FT /Tx /T (ghost) /V (boo) /Rect [50 700 200 720]`. Page 2 `/Annots` contains one **inline** dictionary `<< /Type /Annot /Subtype /Widget /T (inline) /Rect [50 700 200 720] >>`. Expected: 2 `Unknown` entries named `ghost`, `inline`; second `objectId` = `page 2 annot 0`; `hasAcroForm` false.

`acroform-many.pdf` — 30 pages × 50 Tx fields named `f{page}_{i}` (`i` 0..49), rects stacked `[50, 760 - 14*i, 300, 770 - 14*i]`. Expected: 1,500 placements, `formFieldPages` = `[1..30]`.

`acroform-basic-encrypted.pdf` — not generated by the example; produced once with qpdf and committed:
```bash
qpdf --encrypt "" "" 128 -- __test__/fixtures/forms/acroform-basic.pdf __test__/fixtures/forms/acroform-basic-encrypted.pdf
```

- [ ] **Step 1: Write `examples/gen_fixtures.rs`** building each document with `Document::with_version("1.7")`, `dictionary!`, `add_object`, `Object::Reference`, `Stream::new`, `text_string`, and `doc.save(path)`. A small helper per concept (page, widget, text field, button) keeps it short. Output directory `__test__/fixtures/forms/` (create if missing).

- [ ] **Step 2: Generate and sanity-check**

```bash
cargo run --example gen_fixtures
for f in __test__/fixtures/forms/acroform-{basic,inherited,cycle,orphans,many}.pdf; do qpdf --check "$f" >/dev/null && echo "ok $f"; done
qpdf --json --json-key=acroform __test__/fixtures/forms/acroform-basic.pdf | node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>{const j=JSON.parse(s).acroform;console.log(j.fields.filter(f=>f.pageposfrom1).length, j.hasacroform, j.needappearances)})'
qpdf --encrypt "" "" 128 -- __test__/fixtures/forms/acroform-basic.pdf __test__/fixtures/forms/acroform-basic-encrypted.pdf
```
Expected: five `ok` lines; `11 true true`; encrypted file written. (`qpdf --check` may warn about the deliberate cycle fixture; a warning is acceptable, an error is not.)

- [ ] **Step 3: Commit generator and fixtures**

```bash
cargo fmt && cargo clippy --features render --example gen_fixtures -- -D warnings
git add examples/gen_fixtures.rs __test__/fixtures/forms
git commit -m "test(forms): lopdf-generated AcroForm fixtures and generator

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: napi wiring and ava tests for the standalone and class APIs

**Files:**
- Modify: `src/lib.rs` (new sync fn, `AsyncTask` types, `PdfDown` methods), `src/core/mod.rs` (drop `#[allow(dead_code)]`), `src/types/forms.rs` (drop `#[allow(dead_code)]`)
- Create: `__test__/forms.spec.ts`
- Test: `__test__/forms.spec.ts`

**Interfaces:**
- Consumes: `crate::core::forms::{extract_forms, FormsExtraction}`; existing `load_doc`, `PdfDown { doc: Arc<Document>, .. }`.
- Produces (napi, generated names in parentheses):
  - `#[napi] pub fn extract_form_fields_per_page(buffer: Buffer) -> Result<Vec<PageFormField>>` (`extractFormFieldsPerPage`)
  - `pub struct ExtractFormFieldsTask(Vec<u8>)` with `Task { Output = Vec<PageFormField>; JsValue = Vec<PageFormField> }`; `#[napi] pub fn extract_form_fields_per_page_async(buffer: Buffer) -> AsyncTask<ExtractFormFieldsTask>` (`extractFormFieldsPerPageAsync`)
  - `pub struct SharedFormFieldsTask(Arc<Document>)`; `PdfDown::form_fields_per_page(&self) -> Vec<PageFormField>` (`formFieldsPerPage`) and `PdfDown::form_fields_per_page_async(&self) -> AsyncTask<SharedFormFieldsTask>` (`formFieldsPerPageAsync`)
  - All return `extract_forms(doc).fields`.

- [ ] **Step 1: Write the failing ava tests** in `__test__/forms.spec.ts`. Define once at the top: `const FIXTURES = join(import.meta.dirname, "fixtures", "forms");` and `const fixture = (name: string) => readFileSync(join(FIXTURES, name));` (Task 8 reuses both). Test names below are the `test("…")` titles, assertions summarized — assert the exact values from Task 5:

```ts
test("basic: 11 placements, 9 logical fields, reading order")          // names deepEqual ["name","notes","zip","agree","color","color","color","country","tags","sig","submit"]; every page === 1
test("basic: text fields carry flags, limits and strings")             // name: type Text, value "Jane Doe", tooltip "Full name", mappingName "fullName"; notes: multiline true, maxLen 200, value "Line one\nLine two"; zip: comb true, maxLen 5
test("basic: checkbox state comes from /AS")                           // agree: type Checkbox, onState "Ja", checked true, required true, value "Ja"
test("basic: radio group maps /Opt export values onto widgets")        // three entries share fieldObjectId; widgetIndex [0,1,2]; widgetCount 3; onState ["Red","Green","Blue"]; selected only on index 1; value "Green" on all
test("basic: combo, list, push button, signature")                     // country: ComboBox editable true, options [{US,United States},{DE,Germany}], value "US"; tags: ListBox multiSelect true, values ["a","c"], value "a"; submit: PushButton hasActions true; sig: Signature signed false
test("inherited: dotted names, inherited flags, UTF-16 and PDFDocEncoding") // person.first & person.last: Text, readOnly true; last.value "Müller"; "Straße" present; amount.value "42"; badrect.rect.length 3
test("inherited: multi-page placement and duplicate names")            // id: pages [2,3], widgetIndex [0,1], widgetCount 2, same fieldObjectId; dup: two entries with different fieldObjectId
test("cycle: terminates with the single placed field")                 // length 1, name "A.B"
test("orphans: widgets without /AcroForm are Unknown; annotations keep them") // 2 Unknown named ghost, inline; inline.objectId === "page 2 annot 0"; extractAnnotationsPerPage(orphans).filter(a => a.subtype === "Widget").length === 2
test("no form: existing fixture yields an empty array")                // ascii85-dct-scan.pdf → []
test("many: 1,500 placements on 30 pages in page order")               // length 1500; pages non-decreasing; first page 1, last page 30
test("encrypted with empty user password: same fields as plaintext")  // names deepEqual basic names; value "Jane Doe"
test("sync, async and class agree")                                    // for basic + inherited: await extractFormFieldsPerPageAsync deepEqual sync; new PdfDown(buf).formFieldsPerPage() deepEqual; await .formFieldsPerPageAsync() deepEqual
```

- [ ] **Step 2: Build and run to verify they fail**

Run: `yarn build:debug && yarn test __test__/forms.spec.ts`
Expected: FAIL, `extractFormFieldsPerPage is not a function`.

- [ ] **Step 3: Wire `src/lib.rs`** following the `extract_annotations_per_page` / `ExtractAnnotationsTask` / `SharedExtractAnnotationsTask` trio verbatim in shape. Remove the two `#[allow(dead_code)]` attributes.

- [ ] **Step 4: Build, run the new tests and the whole suite**

Run: `yarn build:debug && yarn test`
Expected: all forms tests pass; every pre-existing test still passes.

- [ ] **Step 5: Clippy with warnings denied, format, commit**

```bash
cargo fmt && cargo clippy --features render -- -D warnings && node_modules/.bin/oxfmt --check
git add src/lib.rs src/core/mod.rs src/types/forms.rs __test__/forms.spec.ts index.d.ts index.js
git commit -m "feat(forms): extractFormFieldsPerPage sync/async and PdfDown.formFieldsPerPage

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: `form` on `PdfMeta`, aggregates on `PdfDocument` / `PdfDocumentOcr`

**Files:**
- Modify: `src/types/mod.rs` (`PdfMeta`, `PdfDocument`, `RawPdfDocument` + `From`, and the `ocr`-gated `PdfDocumentOcr`, `RawPdfDocumentOcr` + `From`)
- Modify: `src/core/meta.rs` (`extract_metadata`), `src/core/document.rs` (`extract_all`, `extract_all_with_ocr`)
- Test: `__test__/forms.spec.ts` (append)

**Interfaces:**
- Consumes: `extract_forms`, `form_field_pages`.
- Produces:
  - `PdfMeta.form: Option<FormInfo>` (`form?: FormInfo`)
  - `pub(crate) fn extract_metadata(doc: &Document) -> PdfMeta` now fills `form` from `extract_forms(doc).info`; new `pub(crate) fn extract_metadata_with_form(doc: &Document, form: Option<FormInfo>) -> PdfMeta` does the same work without re-running the form scan
  - `RawPdfDocument` / `RawPdfDocumentOcr` gain `form_fields: Vec<PageFormField>`, `form_field_pages: Vec<u32>`; `PdfDocument` / `PdfDocumentOcr` gain `form: Option<FormInfo>`, `form_fields: Vec<PageFormField>`, `total_form_fields: u32`, `form_field_pages: Vec<u32>` (`form?`, `formFields`, `totalFormFields`, `formFieldPages`)
  - `extract_all` / `extract_all_with_ocr` run `extract_forms` inside the existing `rayon::join` tree (one extra leaf), then call `extract_metadata_with_form(doc, forms.info)`

- [ ] **Step 1: Append the failing ava tests**

```ts
test("pdfMetadata: form info present on forms, absent otherwise")     // basic: form.hasAcroForm true, needAppearances true, fieldCount 9, widgetCount 11, unplacedFields []; inherited: unplacedFields deepEqual [{ fieldObjectId: <string>, name: "hidden", type: "Text", value: "secret" }] (check name/type/value and typeof fieldObjectId); orphans: hasAcroForm false, fieldCount 0, widgetCount 2; ascii85 fixture: form undefined
test("pdfDocument: aggregates mirror images and annotations")         // inherited: totalFormFields === formFields.length === 9; formFieldPages deepEqual [1,2,3]; form deepEqual pdfMetadata(buf).form; formFields deepEqual extractFormFieldsPerPage(buf); ascii85: formFields [], totalFormFields 0, formFieldPages [], form undefined
test("pdfDocumentAsync and PdfDown.document agree on form data")      // async + class variants deepEqual sync pdfDocument for basic
```

- [ ] **Step 2: Build and run to verify they fail**

Run: `yarn build:debug && yarn test __test__/forms.spec.ts`
Expected: FAIL on `form` / `formFields` being undefined.

- [ ] **Step 3: Implement the type additions, `extract_metadata_with_form`, and the `extract_all*` join changes.** `total_form_fields = form_fields.len() as u32` in the `From` impls, mirroring `total_images`.

- [ ] **Step 4: Build, run everything, and compile the OCR feature set**

Run: `yarn build:debug && yarn test && cargo check --features ocr,render`
Expected: all pass; OCR variant compiles (the `RawPdfDocumentOcr` changes are only type-checked here; Task 8 runs the OCR suite).

- [ ] **Step 5: Format, lint, commit**

```bash
cargo fmt && cargo clippy --all-features -- -D warnings && node_modules/.bin/oxfmt --check
git add src/types/mod.rs src/core/meta.rs src/core/document.rs __test__/forms.spec.ts index.d.ts index.js
git commit -m "feat(forms): form info on PdfMeta, formFields aggregates on PdfDocument

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: Type-level test, qpdf cross-check, OCR package parity, docs, CI

**Files:**
- Create: `__test__/forms.types.ts` (not matched by ava's `*.spec.ts` glob; checked by `tsc`)
- Modify: `__test__/forms.spec.ts` (qpdf cross-check), `ocr/__test__/index.spec.ts` (superset test), `README.md`, `ocr/README.md`, `.github/workflows/CI.yml` (add `cargo test` to the lint job after the `cargo clippy` step at line 59), `package.json` (`"typecheck": "tsc --noEmit --strict --module esnext --moduleResolution bundler --types node __test__/forms.types.ts"`)
- Build output: `ocr/index.d.ts`, `ocr/index.js` (regenerated by `yarn build:ocr`)

**Interfaces:**
- Consumes: everything above, as published in `index.d.ts`.

- [ ] **Step 1: Write the type-level assertions** in `__test__/forms.types.ts`:

```ts
import type { FormFieldByType, FormFieldType, IsExact, PageFormField, Rm } from "../index";
const _types: IsExact<FormFieldType, "Text" | "Checkbox" | "Radio" | "ComboBox" | "ListBox" | "PushButton" | "Signature" | "Unknown"> = true;
const _keys: IsExact<keyof FormFieldByType, FormFieldType> = true;
const _checkbox: IsExact<FormFieldByType["Checkbox"], Rm<Extract<PageFormField, { type: "Checkbox" }>, "type">> = true;
const _noType: "type" extends keyof FormFieldByType["Text"] ? false : true = true;
export {};
```

Run: `yarn typecheck` (after adding the script). Expected: exit 0. Flip one literal to confirm it fails, then restore.

- [ ] **Step 2: Add the qpdf cross-check test** to `__test__/forms.spec.ts`:

```ts
const hasQpdf = spawnSync("qpdf", ["--version"]).status === 0;
(hasQpdf ? test : test.skip)("qpdf agrees on placed field names, pages and form flags", (t) => {
  for (const name of ["acroform-basic.pdf", "acroform-inherited.pdf"]) {
    const ours = extractFormFieldsPerPage(fixture(name));
    const theirs = JSON.parse(spawnSync("qpdf", ["--json", "--json-key=acroform", join(FIXTURES, name)], { encoding: "utf8" }).stdout).acroform;
    const placed = theirs.fields.filter((f) => f.pageposfrom1);
    t.deepEqual(ours.map((f) => `${f.page}:${f.name}`).sort(), placed.map((f) => `${f.pageposfrom1}:${f.fullname}`).sort());
    t.is(pdfMetadata(fixture(name)).form?.hasAcroForm, theirs.hasacroform);
    t.is(pdfMetadata(fixture(name)).form?.needAppearances, theirs.needappearances);
  }
});
```

Run: `yarn test __test__/forms.spec.ts`. Expected: passes locally (qpdf 10.6 present); skips where qpdf is absent.

- [ ] **Step 3: OCR package superset test** in `ocr/__test__/index.spec.ts`, next to the existing "base API … works through OCR package" tests:

```ts
test("base API: extractFormFieldsPerPage and documentOcr form aggregates work through OCR package", (t) => {
  const basic = readFileSync(join(import.meta.dirname, "../../__test__/fixtures/forms/acroform-basic.pdf"));
  t.is(extractFormFieldsPerPage(basic).length, 11);
  const doc = pdfDocumentOcr(basic);
  t.is(doc.totalFormFields, 11); t.deepEqual(doc.formFieldPages, [1]); t.is(doc.form?.fieldCount, 9);
});
```

Run: `yarn build:ocr && yarn test:ocr` (needs the local Tesseract 5.5 + pdfium from `PDFIUM_LIBRARY_PATH`; if pdfium is absent locally, the pre-existing `Rendered`-tier tests may skip or fail independently — only the new test is this task's gate). Expected: new test passes; `ocr/index.d.ts` now starts with the shared header and contains `PageFormField`.

- [ ] **Step 4: Docs**

`README.md`: under **API → Standalone functions** add `extractFormFieldsPerPage` / `…Async`; under **`PdfDown` class** add `formFieldsPerPage()` / `formFieldsPerPageAsync()`; under **Types** add `PageFormField` (the union, abbreviated with the "common" comment as in spec §5.1), `ChoiceOption`, `FormInfo`, `UnplacedFormField`, and note `form?: FormInfo` on `PdfMeta` / `PdfDocument` plus `formFields` / `totalFormFields` / `formFieldPages` on `PdfDocument`; add a **Usage → Extract form fields per page** example that narrows on `type` and groups by `fieldObjectId`; add one paragraph **Utility types** explaining the `.d.ts` header (`Rm`, `UTR`, `FormFieldByType`) with a 3-line example; one sentence that the package reads forms and never fills them. `ocr/README.md`: add the two class methods to the method list and one sentence that `PdfDocumentOcr` carries the same `form` / `formFields` aggregates. Code samples use double quotes, semicolons, no trailing commas.

- [ ] **Step 5: CI**

In `.github/workflows/CI.yml`, directly after the `cargo clippy` step (line 59), add a step `name: Unit tests` running `cargo test` (default features; no system libraries needed).

- [ ] **Step 6: Final verification and commit**

```bash
cargo fmt && cargo clippy --all-features -- -D warnings && cargo test
node_modules/.bin/oxfmt --check && node_modules/.bin/taplo format --check && yarn lint && yarn typecheck && yarn test
git add __test__/forms.types.ts __test__/forms.spec.ts ocr/__test__/index.spec.ts README.md ocr/README.md .github/workflows/CI.yml package.json ocr/index.d.ts ocr/index.js
git commit -m "docs(forms): README sections, type-level tests, qpdf cross-check, cargo test in CI

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

## Self-review notes

- Spec coverage: §5.1 API and types → Tasks 1, 6, 7; §5.1.1 → Task 1; §5.2 algorithm → Tasks 2–4; §5.3 edge cases → Task 5 fixtures + Task 6/7 tests (inherited `/FT`, radio with `/Opt`, `Ja` checkbox, comb, multi-select with `/I`, `/Opt` pairs, UTF-16 `/V`, `/Kids` cycle, multi-page field, unplaced field, duplicate names, widgets without `/AcroForm`, indirect and inline `/Annots`, no AcroForm + no widgets, already-filled form reads back (the fixtures *are* filled), widgets still in `extractAnnotationsPerPage`, qpdf cross-check); §9 testing strategy → Tasks 5, 6, 8; §10 phase 1 bullets → Tasks 1, 7, 8; §13 calls → Global Constraints. `hasXfa`, `sigFlags` and `needAppearances` are pinned by the Task 4 unit test `acroform_dictionary_flags_are_reported` (no committed fixture carries `/XFA`).
- Type consistency: `extract_forms` / `FormsExtraction` / `form_field_pages` names are identical in Tasks 4, 6, 7; napi names `extractFormFieldsPerPage[Async]`, `formFieldsPerPage[Async]`, `form`, `formFields`, `totalFormFields`, `formFieldPages`, `PageFormField`, `FormInfo`, `UnplacedFormField`, `ChoiceOption`, `FormFieldType`, `FormFieldByType` match spec §5.1 and §8.
- Review Focus: each of the five has a pinned test (Tasks 2, 4, 6).
