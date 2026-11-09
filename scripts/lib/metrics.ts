import type { InvoiceResult, InvoiceSource } from "../../index";
import type { ExpectedAircraft, ExpectedInvoice, ExpectedItem } from "./expected";
import type { TextClass } from "./fingerprint";

/** Header fields scored by exact match (after normalisation). */
export const HEADER_FIELDS = [
  "invoiceId",
  "invoiceDate",
  "invoiceTotal",
  "amountDue",
  "purchaseOrder",
  "documentType",
  "vendorName",
  "customerName",
  "shippingAddress"
] as const;
export type HeaderField = (typeof HEADER_FIELDS)[number];

/** A set comparison: items or aircraft, as a multiset match with precision and recall. */
export interface SetScore {
  expected: number;
  actual: number;
  matched: number;
  precision: number | null;
  recall: number | null;
}

/** One document compared with its expected file: 1 is a match, 0 a miss, null not stated by the expected file. */
export interface Score {
  /**
   * The magnitude of the expected `invoiceTotal`, the document's weight in the amount-weighted
   * rows (a credit note weighs what it credits); 0 when the expected file states no total.
   */
  weight: number;
  hasAmount: boolean;
  fields: Record<HeaderField, number | null>;
  items: SetScore;
  aircraft: SetScore;
}

export interface DocScore extends Score {
  id: string;
  source: InvoiceSource;
  /** Header fingerprint from `profile.json` when one was available. */
  cluster: string | null;
  textClass: TextClass | null;
  /** Whether the extraction's own items add up to its own net total; null when it has no items or no total. */
  reconciled: boolean | null;
}

export interface FieldAccuracy {
  /** Documents whose expected file states the field. */
  scored: number;
  /** Matches over scored documents, each document counting one. */
  documentWeighted: number | null;
  /** Matches over scored documents, each counting its expected total; documents without one count 0. */
  amountWeighted: number | null;
}

export interface SetAccuracy {
  documents: number;
  precision: number | null;
  recall: number | null;
}

export interface Row {
  label: string;
  documents: number;
  missingAmounts: number;
  amountTotal: number;
  fields: Record<HeaderField, FieldAccuracy>;
  items: SetAccuracy;
  aircraft: SetAccuracy;
  reconciled: { documents: number; rate: number | null };
}

export interface Table {
  all: Row;
  /** One row per header cluster, most documents first. */
  clusters: Array<Row>;
  /** The scanned and mixed subsets, when present. */
  textClasses: Array<Row>;
  /** The caller's id list (`--ids`), when one was given. */
  slice: Row | null;
}

const fieldRecord = <T>(make: (field: HeaderField) => T): Record<HeaderField, T> => ({
  invoiceId: make("invoiceId"),
  invoiceDate: make("invoiceDate"),
  invoiceTotal: make("invoiceTotal"),
  amountDue: make("amountDue"),
  purchaseOrder: make("purchaseOrder"),
  documentType: make("documentType"),
  vendorName: make("vendorName"),
  customerName: make("customerName"),
  shippingAddress: make("shippingAddress")
});

const normText = (s: string): string => s.trim().toLowerCase().replace(/\s+/g, " ");
const cents = (n: number): number => Math.round(n * 100);
const matchText = (actual: string | undefined, expected: string | undefined): number | null =>
  expected === undefined ? null : actual !== undefined && normText(actual) === normText(expected) ? 1 : 0;
const matchAmount = (actual: number | undefined, expected: number | undefined): number | null =>
  expected === undefined ? null : actual !== undefined && cents(actual) === cents(expected) ? 1 : 0;
const ratio = (numerator: number, denominator: number): number | null =>
  denominator === 0 ? null : numerator / denominator;

/**
 * Greedy multiset match: every actual element is matched to at most one expected element with the
 * same key, in order; returns the matched count and the elements left on each side.
 */
function matchByKey<T>(
  actual: ReadonlyArray<T>,
  expected: ReadonlyArray<T>,
  key: (element: T) => string | undefined
): { matched: number; actualLeft: Array<T>; expectedLeft: Array<T> } {
  const pool = new Map<string, Array<T>>();
  const expectedLeft: Array<T> = [];
  for (const e of expected) {
    const k = key(e);
    if (k === undefined) expectedLeft.push(e);
    else pool.set(k, [...(pool.get(k) ?? []), e]);
  }
  let matched = 0;
  const actualLeft: Array<T> = [];
  for (const a of actual) {
    const k = key(a);
    const candidates = k === undefined ? undefined : pool.get(k);
    if (candidates !== undefined && candidates.length > 0) {
      candidates.pop();
      matched += 1;
    } else actualLeft.push(a);
  }
  for (const rest of pool.values()) expectedLeft.push(...rest);
  return { matched, actualLeft, expectedLeft };
}

const setScore = (expected: number, actual: number, matched: number): SetScore => ({
  expected,
  actual,
  matched,
  precision: actual === 0 ? (expected === 0 ? null : 0) : matched / actual,
  recall: expected === 0 ? null : matched / expected
});

const amountKey = (item: ExpectedItem): string => (item.amount === undefined ? "" : String(cents(item.amount)));
const codeKey = (item: ExpectedItem): string | undefined =>
  item.productCode === undefined ? undefined : `${normText(item.productCode)}|${amountKey(item)}`;
const descriptionKey = (item: ExpectedItem): string | undefined =>
  item.description === undefined ? undefined : `${normText(item.description)}|${amountKey(item)}`;

/**
 * Items match on `(productCode, amount)` when both sides carry a code, otherwise on
 * `(description, amount)`; two different codes never match, whatever the descriptions say.
 */
function scoreItems(actual: ReadonlyArray<ExpectedItem>, expected: ReadonlyArray<ExpectedItem> | undefined): SetScore {
  if (expected === undefined) return { expected: 0, actual: actual.length, matched: 0, precision: null, recall: null };
  const byCode = matchByKey(actual, expected, codeKey);
  // Only pairs where at least one side lacks a code may fall back to the description.
  const uncodedLeft = (items: ReadonlyArray<ExpectedItem>) => items.filter((i) => i.productCode === undefined);
  const codedLeft = (items: ReadonlyArray<ExpectedItem>) => items.filter((i) => i.productCode !== undefined);
  const byDescriptionA = matchByKey(uncodedLeft(byCode.actualLeft), byCode.expectedLeft, descriptionKey);
  const byDescriptionB = matchByKey(
    codedLeft(byCode.actualLeft),
    uncodedLeft(byDescriptionA.expectedLeft),
    descriptionKey
  );
  return setScore(expected.length, actual.length, byCode.matched + byDescriptionA.matched + byDescriptionB.matched);
}

const serialKey = (a: ExpectedAircraft): string | undefined =>
  a.serialNumber === undefined ? undefined : normText(a.serialNumber);

function scoreAircraft(
  actual: ReadonlyArray<ExpectedAircraft>,
  expected: ReadonlyArray<ExpectedAircraft> | undefined
): SetScore {
  if (expected === undefined) return { expected: 0, actual: actual.length, matched: 0, precision: null, recall: null };
  return setScore(expected.length, actual.length, matchByKey(actual, expected, serialKey).matched);
}

/**
 * Compare an extraction (projected with `toExpected`) with its expected file. Strings are trimmed,
 * case-folded and whitespace-collapsed; dates compare as `YYYY-MM-DD` strings; amounts to the cent.
 */
export function scoreDocument(actual: ExpectedInvoice, expected: ExpectedInvoice): Score {
  const fields = fieldRecord((field): number | null => {
    switch (field) {
      case "invoiceTotal":
      case "amountDue":
        return matchAmount(actual[field], expected[field]);
      case "shippingAddress":
        return matchText(actual.shippingAddress?.lines.join("\n"), expected.shippingAddress?.lines.join("\n"));
      case "documentType":
        return matchText(actual.documentType, expected.documentType);
      default:
        return matchText(actual[field], expected[field]);
    }
  });
  return {
    weight: Math.abs(expected.invoiceTotal ?? 0),
    hasAmount: expected.invoiceTotal !== undefined,
    fields,
    items: scoreItems(actual.items ?? [], expected.items),
    aircraft: scoreAircraft(actual.aircraft ?? [], expected.aircraft)
  };
}

/**
 * Whether the extraction's item amounts add up to its net total within 1%: the subtotal when the
 * extractor found one, else the total less tax, else the total. Null without items or a total.
 */
export function reconciles(result: InvoiceResult): boolean | null {
  const f = result.fields;
  const amounts = f.items.map((i) => i.amount?.value).filter((a): a is number => a !== undefined);
  if (amounts.length === 0) return null;
  const total = f.invoiceTotal?.value;
  const net = f.subTotal?.value ?? (total !== undefined && f.totalTax !== undefined ? total - f.totalTax.value : total);
  if (net === undefined) return null;
  const sum = amounts.reduce((n, a) => n + a, 0);
  return Math.abs(sum - net) <= Math.abs(net) * 0.01 + 0.005;
}

const setAccuracy = (scores: ReadonlyArray<DocScore>, pick: (s: DocScore) => SetScore): SetAccuracy => {
  const stated = scores.filter((s) => pick(s).expected > 0);
  return {
    documents: stated.length,
    precision: ratio(
      stated.reduce((n, s) => n + pick(s).matched, 0),
      stated.reduce((n, s) => n + pick(s).actual, 0)
    ),
    recall: ratio(
      stated.reduce((n, s) => n + pick(s).matched, 0),
      stated.reduce((n, s) => n + pick(s).expected, 0)
    )
  };
};

function row(label: string, scores: ReadonlyArray<DocScore>): Row {
  const fields = fieldRecord((field): FieldAccuracy => {
    const scored = scores.filter((s) => s.fields[field] !== null);
    const weighted = scored.filter((s) => s.hasAmount);
    return {
      scored: scored.length,
      documentWeighted: ratio(
        scored.reduce((n, s) => n + (s.fields[field] ?? 0), 0),
        scored.length
      ),
      amountWeighted: ratio(
        weighted.reduce((n, s) => n + s.weight * (s.fields[field] ?? 0), 0),
        weighted.reduce((n, s) => n + s.weight, 0)
      )
    };
  });
  const reconciled = scores.filter((s) => s.reconciled !== null);
  return {
    label,
    documents: scores.length,
    missingAmounts: scores.filter((s) => !s.hasAmount).length,
    amountTotal: scores.reduce((n, s) => n + s.weight, 0),
    fields,
    items: setAccuracy(scores, (s) => s.items),
    aircraft: setAccuracy(scores, (s) => s.aircraft),
    reconciled: {
      documents: reconciled.length,
      rate: ratio(reconciled.filter((s) => s.reconciled === true).length, reconciled.length)
    }
  };
}

/** Both weightings for every document, per header cluster, for the scanned and mixed subsets, and for an id slice. */
export function aggregate(scores: ReadonlyArray<DocScore>, options: { ids?: ReadonlySet<string> } = {}): Table {
  const byCluster = new Map<string, Array<DocScore>>();
  for (const s of scores) {
    const key = s.cluster ?? "unclustered";
    byCluster.set(key, [...(byCluster.get(key) ?? []), s]);
  }
  const clusters = [...byCluster.entries()]
    .map(([label, group]) => row(label, group))
    .sort((a, b) => b.documents - a.documents || a.label.localeCompare(b.label));
  const textClasses = (["scanned", "mixed"] as const)
    .map((tc) =>
      row(
        tc,
        scores.filter((s) => s.textClass === tc)
      )
    )
    .filter((r) => r.documents > 0);
  const slice =
    options.ids === undefined
      ? null
      : row(
          `ids slice (${options.ids.size} listed)`,
          scores.filter((s) => options.ids?.has(s.id) ?? false)
        );
  return { all: row("all", scores), clusters, textClasses, slice };
}
