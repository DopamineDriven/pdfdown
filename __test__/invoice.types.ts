import type { IsExact } from "@d0paminedriven/type-utils";

import type {
  AircraftReference,
  Attachment,
  CurrencySource,
  EInvoiceStandard,
  InvoiceDocumentType,
  InvoiceOptions,
  InvoiceResult,
  InvoiceSource,
  MoneyField,
  PageLayout,
  StringField
} from "../index";
import type { extractInvoice, invoiceFromLayout } from "../index";

// shape
true satisfies IsExact<InvoiceSource, "EmbeddedXml" | "Native" | "Ocr" | "Rendered" | "Mixed" | "Spreadsheet" | "None">;
true satisfies IsExact<CurrencySource, "Page" | "Content" | "Option" | "Unknown">;
true satisfies IsExact<InvoiceResult["currencySource"], CurrencySource | undefined>;
true satisfies IsExact<InvoiceResult["creditSignals"], string[]>;
true satisfies IsExact<ReturnType<typeof invoiceFromLayout>, InvoiceResult>;
true satisfies IsExact<
  Parameters<typeof invoiceFromLayout>,
  [pages: PageLayout[], opts?: InvoiceOptions | null | undefined]
>;
true satisfies IsExact<EInvoiceStandard, "FacturX" | "ZUGFeRD" | "XRechnung" | "UBL" | "Unknown">;
true satisfies IsExact<InvoiceDocumentType, "Invoice" | "CreditNote" | "Unknown">;
true satisfies IsExact<NonNullable<InvoiceOptions["decimalSeparator"]>, "Auto" | "." | ",">;
true satisfies IsExact<MoneyField["currencyCode"], string | undefined>;
true satisfies IsExact<Attachment["data"], Buffer>;
true satisfies IsExact<ReturnType<typeof extractInvoice>, InvoiceResult>;

// fields through the result type
true satisfies IsExact<NonNullable<InvoiceResult["fields"]["invoiceTotal"]>["value"], number>;
true satisfies IsExact<NonNullable<InvoiceResult["fields"]["invoiceTotal"]>["text"], string>;
true satisfies IsExact<NonNullable<InvoiceResult["fields"]["vendorAddress"]>["lines"], string[]>;
true satisfies IsExact<NonNullable<InvoiceResult["fields"]["invoiceId"]>["confidence"], number>;

// aviation extension: aircraft references and their engine serials
true satisfies IsExact<InvoiceResult["fields"]["aircraft"], AircraftReference[]>;
true satisfies IsExact<AircraftReference["engineSerialNumber"], StringField | undefined>;
