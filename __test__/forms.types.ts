// Type-level assertions over the generated declarations. Not an ava spec:
// checked by `yarn typecheck` (tsc --noEmit). Flip a literal to see it fail.

import type {
  FormFieldByType,
  FormFieldPayload,
  FormFieldType,
  IsExact,
  PageFormField,
  Rm,
  UnplacedFormField
} from "../index";
import { extractFormFieldsPerPage } from "../index";

// ── Shape ────────────────────────────────────────────────────────────────────

const _types = true satisfies IsExact<
  FormFieldType,
  "Text" | "Checkbox" | "Radio" | "ComboBox" | "ListBox" | "PushButton" | "Signature" | "Unknown"
>;

const _keys = true satisfies IsExact<keyof FormFieldByType, FormFieldType>;

const _checkbox = true satisfies IsExact<
  FormFieldByType["Checkbox"],
  Rm<Extract<PageFormField, { type: "Checkbox" }>, "type">
>;

const _noType = true satisfies "type" extends keyof FormFieldByType["Text"] ? false : true;

const _unplacedType = true satisfies IsExact<UnplacedFormField["type"], FormFieldType>;

// FormFieldPayload: one key gives that variant's payload, a union of keys the union of payloads,
// and no key at all is every payload.
const _sigPayload = true satisfies IsExact<FormFieldPayload<"Signature">, FormFieldByType["Signature"]>;
const _buttonPayload = true satisfies IsExact<
  FormFieldPayload<"Checkbox" | "Radio">,
  FormFieldByType["Checkbox"] | FormFieldByType["Radio"]
>;
const _allPayloads = true satisfies IsExact<FormFieldPayload, FormFieldByType[FormFieldType]>;
const _payloadHasNoType = true satisfies "type" extends keyof FormFieldPayload<"Text"> ? false : true;
declare const sig: FormFieldPayload<"Signature">;
const signed: boolean = sig.signed;
void signed;

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
void _sigPayload;
void _buttonPayload;
void _allPayloads;
void _payloadHasNoType;
