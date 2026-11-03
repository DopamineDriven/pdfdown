import type { Equal, FormFieldByType, FormFieldType, IsExact, PageFormField, Rm, UnplacedFormField } from "../index";
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

// negatives
// @ts-expect-error "type" is stripped from every keyed entry
type _NoDiscriminant = FormFieldByType["Text"]["type"];
// @ts-expect-error checked exists only on the Checkbox variant
type _NoCheckedOnText = FormFieldByType["Text"]["checked"];
