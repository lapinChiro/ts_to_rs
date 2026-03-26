// Test: external type struct generation for builtin types
// When union types reference builtin types (Date, Error, RegExp),
// struct definitions should be generated from TypeRegistry field info.

export type DateOrError = Date | Error;

export function processValue(input: ArrayBuffer | string): string {
    return input.toString();
}
