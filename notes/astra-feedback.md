**Much better—the overall architecture now fits pdfdown. I’d keep it, but fix several correctness issues before implementing the plan literally.**

The page-based arrays, real 1-based page numbers, separate unplaced fields, `objectId`/`fieldObjectId`, preserved annotations, and sync/async/class parity are all aligned with your API. The canonical discriminated union plus derived `FormFieldType` and `FormFieldByType` also fits your conventions.

**1. The logical-field predicate contradicts its own test.**

Task 2 defines `is_logical()` as having an effective field type and no named children.

An unnamed radio widget inherits its parent’s `/FT` and has no children—so **it also satisfies that predicate**. Following this literally could count a radio group and its three widgets as four logical fields, potentially emitting the widgets twice.

The test expects `is_logical(kid) === false`, but the algorithm doesn’t explain how to achieve that. Explicitly distinguish field nodes from widget-only nodes during traversal, including dictionaries that combine both roles. PDF explicitly permits terminal fields to have widget children. [developer.adobe.com](https://developer.adobe.com/document-services/docs/assets/35e4369068f86065372c18787171a17e/PDF_ISO_32000-1.pdf?utm_source=chatgpt.com)

**2. `UnplacedFormField` unnecessarily loses extracted data.**

It retains only identity, name, type, and a single string value.

For an unplaced multiselect field with `/V [(a)(c)]`, the proposed first-element conversion preserves `"a"` and loses `"c"`. Options, defaults, flags, and other logical-field metadata also disappear.

Keep unplaced fields as a discriminated union preserving the applicable **logical-field payload**. Geometry and widget-specific state can be absent without throwing away the underlying field data. A Rust `String` annotated as TypeScript `FormFieldType` also provides less enforcement than an actual enum.

**3. Dereferencing needs to be explicit and consistent.**

`decode_text_object()` cannot resolve references because it receives no `Document`. Meanwhile, the plan treats a signature’s `/V` being *any reference* as sufficient for `signed: true`.

Make callers resolve indirect objects before decoding or classifying them. Add tests for an indirect text value, an indirect multiselect array, and missing or invalid reference targets.

For signatures, inspect the resolved object. Document that `signed` indicates the presence of signature data—not that the signature was cryptographically validated.

**4. Button state names and export values are being conflated.**

In the fixture, `/AP` uses the state name `"1"` while `/Opt` supplies the export value `"Green"`. The plan returns `"Green"` as `onState`, replacing the original state identifier. The specification distinguishes these representations. [developer.adobe.com](https://developer.adobe.com/document-services/docs/assets/35e4369068f86065372c18787171a17e/PDF_ISO_32000-1.pdf?utm_source=chatgpt.com)

I’d preserve `onState: "1"` and expose `exportValue: "Green"` separately.

Also, **don’t fabricate `"Yes"` when the appearance information is missing**. Infer from available evidence where justified, otherwise report the state as unavailable.

The `/AS /Off` versus `/V /Ja` test also deserves an explicit contract: the plan reports `checked: false` while retaining a non-Off field value. Preserve or report that disagreement so consumers can distinguish stored data from appearance state.

**5. Inline-field identity is only partially worked out.**

Task 4 joins widgets to fields exclusively through `ObjectId`. Inline dictionaries have no such ID, so that join cannot connect them.

Task 3’s fallback—using the widget’s ID for an inline field—doesn’t resolve how the widget was matched, and it provides no identity for an unplaced inline field.

Either define a deterministic traversal identity and matching policy for those cases, or explicitly document the limitation. The existing inline-orphan fixture doesn’t exercise this path.

**6. Truncated extraction should be observable.**

Cycle protection and a depth cap are appropriate. But reaching the cap currently produces an ordinary-looking partial result.

Expose a small diagnostic or completeness indicator for depth truncation and unresolved references. For your automation workload, consumers need to distinguish **“nothing was present” from “some content could not be extracted.”**

The verification plan also needs a few adjustments:

| Area | Adjustment |
|---|---|
| Encryption | The proposed 128-bit qpdf command defaults to RC4. Add an explicit AES fixture to substantiate the stated RC4/AES coverage. [QPDF 10.6.3 documentation](https://qpdf.readthedocs.io/en/10.6/cli.html?utm_source=chatgpt.com) |
| Type tests | Add actual narrowing and negative-access assertions, and check both packages’ generated declarations. |
| CI | Explicitly run the new typecheck and the supported WASI build/check; adding `cargo test` alone doesn’t establish those guarantees. |
| Scale | The 1,500-field fixture checks counts and ordering. A separate measurement is needed to support the performance claim. |

**I’d proceed with this architecture after those corrections, with the field/widget classification and unplaced-data preservation addressed first.** Phase 1 can stay focused on AcroForm extraction; invoice grouping and the 896-page processing workflow belong in the subsequent extraction/assembly work.

This is a review of the supplied plan; I haven’t compiled or run its proposed implementation.
