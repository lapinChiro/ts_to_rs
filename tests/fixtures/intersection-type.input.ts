// Two object type literals
type Combined = { name: string } & { age: number };

// Three object type literals
type Triple = { a: string } & { b: number } & { c: boolean };

// With optional field
type WithOptional = { name: string } & { nick?: string };
