function withDefault(value: number | undefined, fallback: number): number {
    return value ?? fallback;
}

function main(): void {
    console.log("has value:", withDefault(42, 0));
    console.log("direct:", 10 ?? 20);
}
