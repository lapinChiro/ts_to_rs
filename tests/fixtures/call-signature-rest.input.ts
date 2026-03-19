// Call signature with rest parameter
interface VarargHandler {
    (...args: number[]): void;
}

// Call signature with mixed params and rest
interface Formatter {
    (template: string, ...values: number[]): string;
}

// Simple call signature (no rest - regression check)
interface Callback {
    (x: number, y: number): number;
}
