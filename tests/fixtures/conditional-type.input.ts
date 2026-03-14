// Type filtering: T extends X ? T : never
export type StringOnly<T> = T extends string ? T : never;

// Simple type conversion: T extends X ? Y : Z
export type ToNumber<T> = T extends string ? number : boolean;

// Type predicate: T extends X ? true : false
export type IsArray<T> = T extends Array<any> ? true : false;

// Infer extraction: T extends Foo<infer U> ? U : never
export type Unwrap<T> = T extends Promise<infer U> ? U : never;

// Unsupported: nested conditional type — should produce fallback
export type Nested<T> = T extends string ? T extends "a" ? number : boolean : never;

// After fallback, subsequent declarations should still convert
export interface AfterFallback {
  x: number;
}
