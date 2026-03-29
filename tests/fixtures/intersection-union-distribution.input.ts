// Base + union → distributed enum
type WithUnion = { base: string } & ({ a: number } | { b: boolean });

// Discriminated union in intersection
type Discriminated = { base: string } & ({ kind: "alpha"; x: number } | { kind: "beta"; y: string });

// Empty variant in union
type WithEmptyVariant = { base: string } & ({ a: number } | {});

// Multiple unions — only first distributed
type MultiUnion = { a: string } & ({ x: number } | { y: number }) & ({ p: boolean } | { q: boolean });

// Duplicate field name between base and variant — variant overrides base
type DuplicateField = { name: string; age: number } & ({ name: number; x: boolean } | { y: string });

// Annotation position: intersection + union
function takeIntersectionUnion(arg: { base: string } & ({ a: number } | { b: boolean })): void {}

// Methods in intersection with union — methods should generate impl block
type WithMethods = { name: string; greet(msg: string): string } & ({ role: "admin"; level: number } | { role: "user"; email: string });
