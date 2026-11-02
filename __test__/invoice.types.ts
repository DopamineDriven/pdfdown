// Type-level assertions for the invoice and attachment surface. Checked by `yarn typecheck`.

import type {
  AircraftReference,
  Attachment,
  EInvoiceStandard,
  InvoiceDocumentType,
  InvoiceOptions,
  InvoiceResult,
  InvoiceSource,
  IsExact,
  MoneyField,
  StringField
} from "../index";
import { extractInvoice } from "../index";

const _source = true satisfies IsExact<InvoiceSource, "EmbeddedXml" | "Native" | "Ocr" | "Rendered" | "Mixed" | "None">;
const _standard = true satisfies IsExact<EInvoiceStandard, "FacturX" | "ZUGFeRD" | "XRechnung" | "UBL" | "Unknown">;
const _docType = true satisfies IsExact<InvoiceDocumentType, "Invoice" | "CreditNote" | "Unknown">;
const _separator = true satisfies IsExact<NonNullable<InvoiceOptions["decimalSeparator"]>, "Auto" | "." | ",">;
const _money = true satisfies IsExact<MoneyField["currencyCode"], string | undefined>;
const _attachmentData = true satisfies IsExact<Attachment["data"], Buffer>;
const _aircraft = true satisfies IsExact<InvoiceResult["fields"]["aircraft"], Array<AircraftReference>>;
const _esn = true satisfies IsExact<AircraftReference["engineSerialNumber"], StringField | undefined>;

declare const buf: Buffer;
const r: InvoiceResult = extractInvoice(buf, { preferEmbeddedXml: true, dateOrder: "Auto" });
const total: number | undefined = r.fields.invoiceTotal?.value;
const exact: string | undefined = r.fields.invoiceTotal?.text;
const lines: Array<string> | undefined = r.fields.vendorAddress?.lines;
void total;
void exact;
void lines;

// @ts-expect-error confidence is a number on every field
const _c: string = r.fields.invoiceId?.confidence;
void _c;

void _source;
void _standard;
void _docType;
void _separator;
void _money;
void _attachmentData;
void _aircraft;
void _esn;
