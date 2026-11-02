// Type-level assertions over the generated declarations. Not an ava spec:
// checked by `yarn typecheck` (tsc --noEmit). Flip a literal to see it fail.

import type { FormFieldByType, FormFieldType, IsExact, PageFormField, Rm, UnplacedFormField } from "../index";
import { extractFormFieldsPerPage } from "../index";

// ── Shape ────────────────────────────────────────────────────────────────────

const _types: IsExact<
  FormFieldType,
  "Text" | "Checkbox" | "Radio" | "ComboBox" | "ListBox" | "PushButton" | "Signature" | "Unknown"
> = true;

const _keys: IsExact<keyof FormFieldByType, FormFieldType> = true;

const _checkbox: IsExact<FormFieldByType["Checkbox"], Rm<Extract<PageFormField, { type: "Checkbox" }>, "type">> = true;

const _noType: "type" extends keyof FormFieldByType["Text"] ? false : true = true;

const _unplacedType: IsExact<UnplacedFormField["type"], FormFieldType> = true;

// ── Narrowing ────────────────────────────────────────────────────────────────

declare const buf: Buffer;

for (const f of extractFormFieldsPerPage(buf)) {
  if (f.type === "Checkbox") {
    const checked: boolean = f.checked;
    const state: string | undefined = f.onState;
    void checked;
    void state;
  }
  if (f.type === "ListBox") {
    const vs: Array<string> | undefined = f.values;
    void vs;
  }
  if (f.type === "Signature") {
    const signed: boolean = f.signed;
    void signed;
  }
  const page: number = f.page;
  const rect: Array<number> = f.rect;
  void page;
  void rect;

  // @ts-expect-error checked exists only on the Checkbox variant
  void f.checked;

  // @ts-expect-error page is a required number on every variant
  const _p: undefined = f.page;
  void _p;
}

void _types;
void _keys;
void _checkbox;
void _noType;
void _unplacedType;
