import type {
  Attachment,
  EInvoiceStandard,
  InvoiceDocumentType,
  InvoiceOptions,
  InvoiceResult,
  InvoiceSource,
  IsExact,
  MoneyField
} from "../index";
import type { extractInvoice } from "../index";

// shape
true satisfies IsExact<InvoiceSource, "EmbeddedXml" | "Native" | "Ocr" | "Rendered" | "Mixed" | "None">;
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
