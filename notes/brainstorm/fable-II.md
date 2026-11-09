**Fable's second pass — 2026-11-05** (on `astra-II.md` and `astra-III.md`)

Andrew has decided: one `PdfDown` class carrying every method for both formats, opened
asynchronously, with public cells, logical pages first, en-US only as a fallback that respects the
format code, and the track pulled ahead of phase IV. Astra's ergonomics stand. This note records
where I land on her ten sharpenings and her workbook facts; the consolidated design is
`notes/2026-11-05/*.md` and is what the plan is written from.

**Taken as written:** sheet identity on every cell join (the page carries `sheet`, the word carries
`cell`, so the pair is the key); one logical page per selected sheet with breaks kept as metadata;
typed dates carried through to the engine once a cell is associated with a field, `DateOrder` only
for genuinely ambiguous text; the en-US fallback never reorders a stated date code, never moves a
quoted symbol, never reads `$` as USD; Strict OOXML and macro or template workbooks reported as
unsupported until a real fixture exists; `detectFormat` returns an object with a detail, and
`openAsync` fails with the same detail; word boxes come from an estimated text extent placed by
alignment, never stretched across a merge; hidden sheets and hidden cells are separate options and
omitted drawings are reported; limits read one byte past the cap and reject the overflow; fixtures
cover Deflate as well as Stored.

**From the inspection, now requirements rather than guesses:** every styled cell in the sample is
behind an `<xf>` with children, so the style parser must count start events; 723 of 837 formula
records are shared, so followers carry a master reference and a cached result and no expression;
269 caches are explicitly empty strings and are cached text, not zero and not uncalculated; cell
type beats number format; vertical centering, wrapping and indentation are everywhere, so the
projection honours alignment from the first slice; the Normal font is Calibri 11 while cell text is
Arial 8 to 26, so column widths use the Normal font and text extents use the cell font; the real
format set is small and includes a quoted unit literal that must not read as a clock.

**Two places I would still hold a different line.** The formatter subset belongs in the second
slice, not after projection: the sample's seven codes are a day of work and every comparison with
a twin depends on display text. And `invoiceAsync` on a workbook should return the same shape a
PDF without embedded XML returns today, `source: "None"` with a warning, rather than an error,
because the consumer's loop should not need a format branch to survive the months before the
shared engine exists.
