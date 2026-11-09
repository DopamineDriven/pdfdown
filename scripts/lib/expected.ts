import { readFileSync } from "fs";

import type { InvoiceDocumentType, InvoiceResult } from "../../index";

/**
 * The comparable subset of `InvoiceFields`, values only: what an `<id>.expected.json` states and
 * what `toExpected` projects an extraction onto. Every key is optional; a key the expected file
 * does not state is not scored.
 */
export interface ExpectedItem {
  productCode?: string;
  description?: string;
  quantity?: number;
  unitPrice?: number;
  amount?: number;
}

export interface ExpectedAircraft {
  serialNumber?: string;
}

export interface ExpectedInvoice {
  invoiceId?: string;
  /** `YYYY-MM-DD`. */
  invoiceDate?: string;
  invoiceTotal?: number;
  amountDue?: number;
  purchaseOrder?: string;
  documentType?: InvoiceDocumentType;
  vendorName?: string;
  customerName?: string;
  shippingAddress?: { lines: Array<string> };
  items?: Array<ExpectedItem>;
  aircraft?: Array<ExpectedAircraft>;
}

/** `<id>.expected.json` sits beside `<id>.pdf`. */
export const expectedPathFor = (pdfPath: string): string => pdfPath.replace(/\.pdf$/i, ".expected.json");

/** Project an extraction onto the comparable subset: the `.value` of each field, items by value. */
export function toExpected(result: InvoiceResult): ExpectedInvoice {
  const f = result.fields;
  return {
    ...(f.invoiceId === undefined ? {} : { invoiceId: f.invoiceId.value }),
    ...(f.invoiceDate === undefined ? {} : { invoiceDate: f.invoiceDate.value }),
    ...(f.invoiceTotal === undefined ? {} : { invoiceTotal: f.invoiceTotal.value }),
    ...(f.amountDue === undefined ? {} : { amountDue: f.amountDue.value }),
    ...(f.purchaseOrder === undefined ? {} : { purchaseOrder: f.purchaseOrder.value }),
    documentType: result.documentType,
    ...(f.vendorName === undefined ? {} : { vendorName: f.vendorName.value }),
    ...(f.customerName === undefined ? {} : { customerName: f.customerName.value }),
    ...(f.shippingAddress === undefined ? {} : { shippingAddress: { lines: f.shippingAddress.lines } }),
    items: f.items.map((i): ExpectedItem => ({
      ...(i.productCode === undefined ? {} : { productCode: i.productCode.value }),
      ...(i.description === undefined ? {} : { description: i.description.value }),
      ...(i.quantity === undefined ? {} : { quantity: i.quantity.value }),
      ...(i.unitPrice === undefined ? {} : { unitPrice: i.unitPrice.value }),
      ...(i.amount === undefined ? {} : { amount: i.amount.value })
    })),
    aircraft: f.aircraft.map((a): ExpectedAircraft =>
      a.serialNumber === undefined ? {} : { serialNumber: a.serialNumber.value }
    )
  };
}

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const isDocumentType = (v: unknown): v is InvoiceDocumentType =>
  v === "Invoice" || v === "CreditNote" || v === "Unknown";

const TEXT_KEYS = ["invoiceId", "invoiceDate", "purchaseOrder", "vendorName", "customerName"] as const;
const AMOUNT_KEYS = ["invoiceTotal", "amountDue"] as const;
const ITEM_TEXT_KEYS = ["productCode", "description"] as const;
const ITEM_NUMBER_KEYS = ["quantity", "unitPrice", "amount"] as const;
const KNOWN_KEYS: ReadonlySet<string> = new Set([
  ...TEXT_KEYS,
  ...AMOUNT_KEYS,
  "documentType",
  "shippingAddress",
  "items",
  "aircraft"
]);

function parseItem(value: unknown, at: string, warnings: Array<string>): ExpectedItem | undefined {
  if (!isRecord(value)) {
    warnings.push(`${at}: not an object`);
    return undefined;
  }
  const item: ExpectedItem = {};
  for (const key of ITEM_TEXT_KEYS) {
    const v = value[key];
    if (typeof v === "string") item[key] = v;
    else if (v !== undefined) warnings.push(`${at}.${key}: expected a string`);
  }
  for (const key of ITEM_NUMBER_KEYS) {
    const v = value[key];
    if (typeof v === "number") item[key] = v;
    else if (v !== undefined) warnings.push(`${at}.${key}: expected a number`);
  }
  return item;
}

/**
 * Read an expected file's JSON into `ExpectedInvoice`, keeping only known keys with the right
 * types; every dropped key or value is a warning, never an exception.
 */
export function parseExpected(json: unknown): { expected: ExpectedInvoice; warnings: Array<string> } {
  const warnings: Array<string> = [];
  const expected: ExpectedInvoice = {};
  if (!isRecord(json)) {
    warnings.push("expected file is not a JSON object");
    return { expected, warnings };
  }
  for (const key of Object.keys(json)) {
    if (!KNOWN_KEYS.has(key)) warnings.push(`${key}: not an InvoiceFields key this harness compares`);
  }
  for (const key of TEXT_KEYS) {
    const v = json[key];
    if (typeof v === "string") expected[key] = v;
    else if (v !== undefined) warnings.push(`${key}: expected a string`);
  }
  for (const key of AMOUNT_KEYS) {
    const v = json[key];
    if (typeof v === "number") expected[key] = v;
    else if (v !== undefined) warnings.push(`${key}: expected a number`);
  }
  if (json.documentType !== undefined) {
    if (isDocumentType(json.documentType)) expected.documentType = json.documentType;
    else warnings.push("documentType: expected Invoice, CreditNote or Unknown");
  }
  if (json.shippingAddress !== undefined) {
    const address = json.shippingAddress;
    if (isRecord(address) && Array.isArray(address.lines) && address.lines.every((l) => typeof l === "string")) {
      expected.shippingAddress = { lines: address.lines.map(String) };
    } else warnings.push("shippingAddress: expected { lines: string[] }");
  }
  if (json.items !== undefined) {
    if (Array.isArray(json.items)) {
      const items: Array<ExpectedItem> = [];
      json.items.forEach((item, i) => {
        const parsed = parseItem(item, `items[${i}]`, warnings);
        if (parsed !== undefined) items.push(parsed);
      });
      expected.items = items;
    } else warnings.push("items: expected an array");
  }
  if (json.aircraft !== undefined) {
    if (Array.isArray(json.aircraft)) {
      const aircraft: Array<ExpectedAircraft> = [];
      json.aircraft.forEach((entry, i) => {
        if (!isRecord(entry)) warnings.push(`aircraft[${i}]: not an object`);
        else if (entry.serialNumber === undefined) aircraft.push({});
        else if (typeof entry.serialNumber === "string") aircraft.push({ serialNumber: entry.serialNumber });
        else warnings.push(`aircraft[${i}].serialNumber: expected a string`);
      });
      expected.aircraft = aircraft;
    } else warnings.push("aircraft: expected an array");
  }
  return { expected, warnings };
}

/** The expected file at `path`, parsed leniently; use `parseExpected` to see the warnings. */
export function readExpected(path: string): ExpectedInvoice {
  return parseExpected(JSON.parse(readFileSync(path, "utf8"))).expected;
}
