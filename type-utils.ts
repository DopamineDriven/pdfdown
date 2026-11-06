export type Unenumerate<T> = T extends (infer U)[] | readonly (infer U)[] ? U : T;

/**
 * Superior form of Omit
 */
export type Rm<T, P extends keyof T = keyof T> = {
  [S in keyof T as Exclude<S, P>]: T[S];
};
/**
 * helper workup for use in XOR type below
 * makes properties from U optional and undefined in T, and vice versa
 */
export type Without<T, U> = { [P in Exclude<keyof T, keyof U>]?: never };

export type Include<T, U extends T> = Exclude<T, Exclude<T, U>>;

/**
 * enforces mutual exclusivity of T | U
 */
// prettier-ignore
export type XOR<T, U> =
  [T, U] extends [object, object]
    ? ((Without<T, U> & U) | (Without<U, T> & T))
    : T | U

export type IsOptional<T, K extends keyof T> = undefined extends T[K]
  ? object extends Pick<T, K>
    ? true
    : false
  : false;

export type OnlyOptional<T> = {
  [K in keyof T as IsOptional<T, K> extends true ? K : never]: T[K];
};

export type OnlyRequired<T> = {
  [K in keyof T as IsOptional<T, K> extends false ? K : never]: T[K];
};
/**
 * CTR (Conditional to Required)
 *
 * - By default: makes all **optional** properties required.
 * - With K: makes only the specified optional keys required.
 */
export type CTR<T, K extends keyof OnlyOptional<T> = keyof OnlyOptional<T>> = Rm<T, K> & {
  [Q in K]-?: T[Q];
};

/**
 * RTC (Required to Conditional)
 *
 * - By default: makes all **required** properties optional.
 * - With K: makes only the specified required keys optional.
 */
export type RTC<T, K extends keyof OnlyRequired<T> = keyof OnlyRequired<T>> = Rm<T, K> & {
  [Q in K]?: T[Q];
};

/**
 * AtLeastOne
 *
 * - every property optional, but at least one of K must be present
 * - By default: K is every key of T
 */
export type AtLeastOne<T, K extends keyof OnlyOptional<RTC<T>> = keyof OnlyOptional<RTC<T>>> = {
  [P in K]: CTR<RTC<T>, P>;
}[K];

export type IsExact<T, U> = [T] extends [U] ? ([U] extends [T] ? true : false) : false;

export type UTR<
  TUnion extends Record<TKey, string>,
  TKey extends string = "kind",
  TExclude extends `strip-${TKey}` | boolean = false
> = {
  [K in TUnion[TKey]]: TExclude extends false
    ? Extract<TUnion, Record<TKey, K>>
    : Rm<Extract<TUnion, Record<TKey, K>>, TKey>;
};

export type NumRange<N extends number = number, Result extends unknown[] = []> = Result["length"] extends N
  ? Result
  : NumRange<N, [...Result, Result["length"]]>;

export type NumUnion<A extends number> = Unenumerate<NumRange<A>>;

export type RepeatRO<T, N extends number, Result extends readonly T[] = readonly []> = N extends N
  ? Result["length"] extends N
    ? Result
    : RepeatRO<T, N, readonly [...Result, T]>
  : never;

export type Repeat<T, N extends number, Result extends T[] = []> = N extends N
  ? Result["length"] extends N
    ? Result
    : Repeat<T, N, [...Result, T]>
  : never;

export type FlexiRepeater<
  T,
  RO extends boolean = boolean,
  Min extends number = 1,
  Max extends number = 5
> = RO extends true
  ? RepeatRO<T, Exclude<NumUnion<Max> | Max, NumUnion<Min>>>
  : Repeat<T, Exclude<NumUnion<Max> | Max, NumUnion<Min>>>;
  
