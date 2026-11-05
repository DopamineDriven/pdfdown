**Astra's second pass — 2026-11-04**

Yes: one public class with async methods can handle both PDF and XLSX, and that is what I would recommend for your Node/TypeScript server. Keep `PdfDown` as the public entry point, give it an internal document-format variant, and dispatch its common operations internally. Your application should be able to open either format and call the same layout and, eventually, invoice methods.

I read [Fable's response](fable-init.md), revisited the class and task implementations in [src/lib.rs](../../src/lib.rs), and read the linked ZIP bridge example. This note proposes a contract; it does not implement it. Your requirement for one class is settled by your message. The other answers below are recommendations for you to consider, rather than decisions attributed to you.

**The current class is immutable in use, rather than stateless.** `PdfDown` stores both `doc: Arc<Document>` and `raw: Arc<Vec<u8>>`. Its constructor parses a specific buffer, and its methods read that document. The async methods clone the `Arc` into a task, so repeated calls share the parsed document instead of reparsing the input. This is useful state.

I would preserve that ownership model: one class type for all supported formats, one instance per opened document, read-only extraction methods, and per-call options. Multiple requests can own independent instances of the same class. A document instance should not have a mutable “current file” or “selected sheet” that one request can change while another extraction is running.

A stateless service class is also possible: every method could accept a buffer and return a result. Its cost is that separate layout, cells, and invoice calls ordinarily parse that buffer separately unless we add a cache or a combined operation. Your existing per-document design already avoids that cost. Being class-based does not require a singleton shared by every websocket connection.

**The async requirement fits the machinery already in the repository.** There are three separate concerns: the JavaScript API returns a Promise, expensive work runs off the JavaScript thread, and the ZIP reader itself performs synchronous operations. These are compatible. `SharedLayoutTask::compute` already does synchronous Rust work on a worker; `PdfDown.layoutAsync()` exposes the Promise to the caller. NAPI-RS documents that `compute` runs on a libuv worker and `resolve` returns to the JavaScript thread. [NAPI-RS AsyncTask documentation](https://napi.rs/docs/concepts/async-task).

The linked [ZIP bridge example](https://github.com/zip-rs/zip2/blob/master/examples/async_bridge.rs) creates a ZIP writer on a dedicated thread and forwards chunks through a bounded channel. It demonstrates streaming output with backpressure. For this project's complete input buffers, the applicable idea is to offload the synchronous ZIP/XML work. We can do that with the existing N-API tasks. Neither Tokio nor `async_zip` is required for async methods on the public class.

There is one important gap in today's API: `new PdfDown(buffer)` parses the PDF synchronously before any async method runs. For your server, I would add an asynchronous opening method:

```ts
// Proposed API; openAsync and spreadsheetCellsAsync do not exist yet.
const document = await PdfDown.openAsync(buffer);

const pages = await document.layoutAsync(); // PDF or XLSX

if (document.format === "Xlsx") {
  const cells = await document.spreadsheetCellsAsync();
}

// Once the shared layout-based invoice engine is implemented:
const invoice = await document.invoiceAsync(invoiceOptions);
```

`openAsync` should perform format detection, archive traversal, decompression, and parsing inside the worker. Resolve the finished Rust state into a `PdfDown` instance on the JavaScript thread. Retain the existing synchronous constructor for callers that deliberately choose synchronous opening. Both opening paths should use the same internal parser and limits.

This can be an ordinary static N-API method returning `AsyncTask<OpenDocumentTask>`, with the task resolving to the class. It does not require a Rust `async fn`. I checked the installed task trait and generated class-conversion machinery: the pieces exist. Validate the generated `Promise<PdfDown>` signature and actual class-instance return in a small binding test during implementation; this sketch has not been compiled. In particular, do not assume putting `#[napi(factory)]` on a function returning `AsyncTask` is interchangeable with returning the class itself.

Input copying and conversion of large results into JavaScript objects still have costs on the calling/completion thread. Async opening moves the parsing work; it does not promise that every byte transfer becomes free.

**One public class should encapsulate separate internal readers.** A minimal conceptual Rust shape is:

```rust
// Architectural sketch, not drop-in implementation.
enum ParsedDocument {
  Pdf(PdfState),              // lopdf document and retained PDF bytes
  Spreadsheet(WorkbookState), // owned cells, styles, package facts
}

pub struct PdfDown {
  inner: Arc<ParsedDocument>,
}
```

The class dispatches on that enum. PDF decoding stays in the existing PDF modules; workbook reading stays in `core::spreadsheet`. The shared invoice engine receives layouts and structured evidence through one internal interface. Nothing requires a second exported `Workbook` class or a format switch in your server's main extraction flow.

Parse into owned workbook data, so the class does not retain XML nodes borrowed from a temporary string or share a mutable ZIP cursor across async calls. Retain the original workbook bytes only if the chosen lazy-loading or inspection contract needs them. Add a `Send + Sync` assertion for the shared state, following the assertion already used for `lopdf::Document`.

Parsing once and computing layout once are separate promises. Initially, share the parsed document and avoid introducing an unlimited result cache. If layout caching is useful, key it by the options that affect the output: sheet selection, visibility, formatting locale, and projection mode. The invoice engine should access workbook evidence directly from this state; it should not serialize cells to JavaScript and then ask the caller to pass them back.

**The class's methods need an explicit applicability contract.** A single class can expose common operations and format-specific operations without fabricating PDF metadata for a workbook.

| Method or property                              | Recommended behavior                                                                                         |
| ----------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| `openAsync(buffer)` and synchronous constructor | Detect and open either supported format; reject malformed or unsupported input with a useful reason.         |
| Read-only `format`                              | Report the opened format without parsing again.                                                              |
| `layoutAsync(options?)`                         | Dispatch to native PDF layout or spreadsheet projection and return the common `PageLayout[]` shape.          |
| `invoiceAsync(options?)`                        | Keep the PDF embedded-XML path; add shared layout-based extraction for both formats when that engine exists. |
| `spreadsheetCellsAsync(options?)`               | Return structured cells for a workbook; report an unsupported operation on a PDF.                            |
| Existing PDF-specific methods                   | Preserve their existing PDF return types and behavior. Explicitly reject unsupported workbook operations.    |
| A new common metadata method, if needed         | Return a discriminated result such as `{ format: "Pdf", pdf: ... }` or `{ format: "Xlsx", workbook: ... }`.  |

For example, `metadataAsync()` currently returns `PdfMeta`, and `documentAsync()` returns `PdfDocument`. Quietly widening those types to unrelated workbook objects would break existing TypeScript consumers. Add a common metadata method when needed, or make an intentional versioned API change. The requirement for one class does not require every existing PDF method to acquire an XLSX meaning. Empty arrays should continue to mean a supported extraction found nothing, rather than quietly signaling that an operation was unsupported.

For websocket use, keep document instances scoped to jobs or an explicitly bounded cache. Worker tasks retain their own `Arc`, so they can finish safely if the JavaScript instance is collected. Bound concurrent jobs: the libuv pool is shared, and this repository also uses Rayon and an OCR pool. If cancellation is exposed, distinguish canceling queued work from cooperatively stopping work already running; rejecting a Promise or losing a socket does not automatically interrupt inflation or parsing. Native and WASI async behavior should both be tested through their supported hosts.

**These are the answers I would give to Fable's five questions.**

| Question                                       | Suggested answer                                                                          | Reason                                                                                                                                                                              |
| ---------------------------------------------- | ----------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Bring the spreadsheet track ahead of phase IV? | Yes, start the reader, cell API, formatter, projection, and class integration now.        | These have independent fixtures and useful outputs. Spreadsheet invoice extraction still depends on the shared semantic engine and its evaluation gate.                             |
| Make structured cells public initially?        | Yes, as a separate result from the same class, with async access.                         | A server consumer benefits from inspecting cached values, formulas, display text, and cell addresses. Keep the richer internal model private and expose a deliberate stable subset. |
| Require printed-page parity immediately?       | Begin with logical layout and correct values/associations; defer print pagination parity. | This produces a useful extractor without making printer-dependent layout a prerequisite. State exactly what a logical page means.                                                   |
| Fixed en-US formatting?                        | Use en-US as a documented fallback where the supported format leaves a choice.            | Preserve explicit format literals and ordering. Report unsupported locale features; do not silently rewrite everything into US presentation or infer USD.                           |
| A Workbook class or standalone functions?      | One public `PdfDown` class supporting both formats, with async opening and methods.       | This is your stated requirement. Standalone functions may remain convenience wrappers over the same internals.                                                                      |

On public cells, this revises my initially conservative position. Fable's separate cell result avoids putting ambiguous values on OCR/PDF words, and your class-based consumer has a clear use for inspecting the parsed workbook. The class also avoids the repeated parsing that separate buffer-taking functions would otherwise encourage. I would settle the public cell schema before implementing it: preserve raw numeric spelling, supported normalized values, display text and its quality, formula-cache status, merge ownership, and source identity. “Non-empty” must include formula cells with missing caches and cells with errors, even when no visible text fragment is emitted.

**Fable's proposal has a strong implementation sequence, with several contracts I would sharpen.** I agree with formatting before projection fidelity, explicit public expectations, logical pages first, no OCR confidence on spreadsheet words, and one semantic engine with an evidence interface. I would adjust the following details before turning the note into a plan:

1. **Cell references need sheet identity.** `B7` occurs on every sheet. Within a document, join by `(sheet identity, cell reference)`, with a human-readable sheet name available. Across documents, the document identity also matters. `Word.cell: Option<usize>` can carry an internal ID through sorting, but `words_to_lines` must explicitly preserve it when constructing public words. A range-derived field may refer to several cells, so `FieldSpan.cell?: string` alone cannot express every provenance case.

2. **One logical page per sheet and honoring page breaks are different contracts.** Fable proposes both. For the first release I would choose one logical page per selected sheet or supported selected print range, retain manual breaks as metadata, and avoid splitting. A later print mode can interpret breaks with fit settings and repeated titles. This also makes page numbering and cell joins easier to specify.

3. **Preserve normalized typed dates through semantic extraction.** Fable suggests storing an ISO value, then making the invoice parser read the displayed text with `DateOrder`. Once a cell has a supported date format and a valid serial/date interpretation, that would discard stronger evidence. A cell displaying `03/04/26` need not be guessed again if its stored value already establishes the date. Associate the cell with an invoice field using the shared engine, then use its supported typed value; preserve display text as evidence. Apply `DateOrder` when interpreting actual ambiguous text. Keep serial 60 and unsupported formats explicitly unresolved.

4. **An en-US fallback must respect the format code.** Preserve a specified date order, quoted currency symbol, and symbol position. It should not turn `dd/mm/yyyy` into `mm/dd/yyyy`, move an explicitly suffixed currency symbol, or make `$` mean USD. Numeric XML values, display locale, and document currency are different concerns. For unsupported locale directives, retain the value and code and report the limitation. [Microsoft's number-format definition](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.spreadsheet.numberingformat).

5. **Strict OOXML support requires more than local-name matching.** Relationship types and namespaced attributes must also be recognized correctly. Microsoft documents explicit namespace mapping when its SDK loads Strict files. Recognize the supported namespace families deliberately and add a real Strict fixture before claiming support. Matching arbitrary namespaces by local element name is too permissive. [Microsoft's Strict-format support description](https://learn.microsoft.com/en-us/office/open-xml/about-the-open-xml-sdk).

6. **Macro-enabled and template support is a proposed scope expansion.** It may be practical to read their ordinary worksheet parts with the same internal backend. It still needs distinct content-kind reporting, fixtures, and a clear cached-value-only contract. For the first milestone I favor the previously agreed XLSX scope. Recognize other package kinds with an explicit unsupported reason, then add support deliberately. An OLE signature alone also does not prove that a file is an encrypted Office document.

7. **A format string cannot also carry diagnostic details.** `detectFormat(buffer) -> "Pdf" | "Xlsx" | "Unknown"` cannot return the promised reason unless another result channel exists. For the class workflow, let `openAsync` do detection and provide a structured failure. A separate inspection API, if useful, should return a result object with format, package kind, and diagnostics. Callers should not have to detect and then open, duplicating the archive work.

8. **Proportional word boxes need an independent estimate of text width.** Do not stretch a short label across a wide merged cell just because that is the available rectangle. Estimate text width from the font-size policy, place the text using alignment, and divide that extent into word fragments. `0.5 em` and a fixed maximum-digit width are acceptable documented approximations for a first subset. They are not accurate font measurement. Wrapping, overflowing text, CJK, and rich runs need explicit limitations and fixtures. In the existing PDF path, available glyph or standard-font metrics already take precedence over a generic estimate.

9. **Visibility and omitted graphics must be observable.** A single `includeHidden` flag is ambiguous across sheets, rows, and columns. Specify its scope or use separate options. Hidden dimensions contribute zero extent in a visible projection; an explicit all-content projection needs a policy for showing them. `images: []` is acceptable when images are absent or intentionally unsupported, but a workbook with drawing content should report that omission. Successful cell extraction does not prove all visible content was extracted.

10. **Byte limits need an overflow check.** `Read::take(limit)` alone can silently end the stream at the cap. Read up to `limit + 1` with checked arithmetic and reject an overflow, or use an equivalent counting reader; distinguish that from valid EOF. Retain archive-input, indexing, aggregate, XML-node, and projected-output limits. Exercise both Stored and Deflate entries, since fixtures written only as Stored would not validate the enabled decoder path.

The narrower `deflate-flate2-zlib-rs` feature Fable suggests is a reasonable small dependency change: the installed manifest shows that the broader `deflate` feature also enables Zopfli. Make it with native/WASI verification and a Deflate-compressed fixture, rather than treating manifest inspection as a build result.

**The next implementation slice should prove the unified class before building the entire formatter.** First refactor PDF ownership behind the internal enum while keeping existing PDF behavior and tests. Add async opening and verify that it resolves to an actual `PdfDown` instance. Then add a minimal bounded workbook reader and one synthetic worksheet, and exercise `openAsync → spreadsheetCellsAsync → layoutAsync` on the same instance. Confirm that calls reuse parsed data and that separate instances and simultaneous reads cannot interfere.

After that, expand the formatter and projection fixtures in Fable's order, extend generated declarations for both packages, and verify WASI. Test format-specific method failures and the existing PDF methods as part of the class refactor. The planned invoice engine can then consume both sources through the same public `invoiceAsync` method and the same internal interpretation path.
