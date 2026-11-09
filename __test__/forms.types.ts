import type {
  Equal,
  FormFieldByType,
  FormFieldPayload,
  FormFieldType,
  IsExact,
  PageFormField,
  Rm,
  UnplacedFormField
} from "../index";
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
true satisfies Equal<FormFieldByType["Checkbox"]["checked"], boolean>;
true satisfies IsExact<FormFieldByType["Checkbox"]["onState"], string | undefined>;
true satisfies IsExact<FormFieldByType["ListBox"]["values"], string[] | undefined>;
true satisfies IsExact<FormFieldByType["Text"]["page"], number>;
true satisfies IsExact<FormFieldByType["Text"]["rect"], number[]>;

// FormFieldPayload: one key gives that variant's payload, a union of keys the union of payloads,
// no key is every payload, and "type" is never present
true satisfies Equal<FormFieldPayload<"Signature">, FormFieldByType["Signature"]>;
true satisfies Equal<FormFieldPayload<"Checkbox" | "Radio">, FormFieldByType["Checkbox"] | FormFieldByType["Radio"]>;
true satisfies Equal<FormFieldPayload, FormFieldByType[FormFieldType]>;
true satisfies Equal<FormFieldPayload<"Signature">["signed"], boolean>;

// negatives
// @ts-expect-error "type" is stripped from every keyed entry
type _NoDiscriminant = FormFieldByType["Text"]["type"];
// @ts-expect-error checked exists only on the Checkbox variant
type _NoCheckedOnText = FormFieldByType["Text"]["checked"];
// @ts-expect-error "type" is stripped from every payload
type _NoTypeOnPayload = FormFieldPayload<"Text">["type"];
