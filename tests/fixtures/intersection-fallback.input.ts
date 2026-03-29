// Mapped type member in intersection → embedded field
type WithMapped<T> = { x: string } & { [K in keyof T]: T[K] };

// Conditional type member in intersection → embedded field
type WithConditional<T, U> = { x: string } & (T extends U ? { y: number } : { z: boolean });
