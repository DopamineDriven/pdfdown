_For phase I=E2=80=99s role=E2=80=94read-only AcroForm extraction that foll=
ows your
existing API=E2=80=94yes, it now captures the core requirements._ I=E2=80=
=99d keep this
scope rather than pulling the other phases into it.

It covers the shape you asked for:

- Flat, page-associated field entries with rectangles and metadata.
- Multiple fields per page and multiple placements of one logical field.
- formFields, formFieldPages, and totals consistent with your existing
  extraction pattern.
- Object identities for distinguishing fields and joining their
  placements.
- Field values, options, flags, defaults, and button states.
- Both packages, standalone/class APIs, sync/async parity, and WASI
  exports.
- Unplaced fields, malformed-input diagnostics, and regression fixtures.

_The important boundary is that this extracts existing PDF form fields._
Your new WHMCS invoice fixture makes that explicit: it contains an empty
/AcroForm, so phase I correctly returns an empty formFields array.
Extracting that invoice=E2=80=99s actual business data still requires the l=
ater
work.

Across the remaining phases, these requirements need explicit ownership:

_Your requirement_

_Work still needed beyond phase I_

Hundreds/thousands of invoice layouts

Positioned text, table reconstruction, and invoice interpretation

Scanned invoices

OCR text and geometry feeding the same extraction pipeline

Multiple invoices in one PDF

Invoice boundaries, continuation detection, and grouping

One invoice spanning 896 pages

Bounded processing, progress/recovery, and complete line-item assembly

Native XLSX/DOCX, eventually PPTX

Office parsers preserving source structure without mandatory PDF conversion

I only have the updated phase-I plan here, so I can=E2=80=99t confirm that =
phases
II=E2=80=93IV now include all of those=E2=80=94particularly _Office extract=
ion and
multi-invoice handling_, which the earlier overall design excluded.

There are _three concrete inconsistencies_ I=E2=80=99d still correct in thi=
s phase:

1.  _The missing-appearance checkbox rule disagrees with its test._
    The algorithm falls back to comparing /V with the widget=E2=80=99s onSta=
    te. But
    the test has no /AP, therefore no onState, and still expects checked:
    true from /V /Ja. Define that fallback explicitly. Radio buttons need
    separate treatment because the group value alone may not identify a widg=
    et
    whose state name is unknown.
2.  _The inline-widget fixture assigns the widget=E2=80=99s synthetic identity to
    the logical field._
    Its parent is the logical field at 5 0 obj; the unnamed inline child is
    its widget. The unplaced logical field should retain fieldObjectId: "5 0
    obj". The proposed expectation "5 0 obj/kid 0" contradicts that
    distinction.
3.  _The unplaced record still doesn=E2=80=99t preserve every applicable =
    datum._
    A flat record is a workable design choice, but it lacks signed.
    Consequently, an unplaced signature field loses the signature-presence
    information exposed for placed fields. Add a fixture for that case.

Those are corrections within the current scope. _The phase-I API direction
is now aligned with what you want; the remaining phases must deliver the
broader invoice-processing capabilities._
