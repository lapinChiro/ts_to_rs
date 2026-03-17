function withDefault(value: number | undefined, fallback: number): number {
    return value ?? fallback;
}

function stringFallback(s: string | undefined): string {
    return s ?? "default";
}

function main(): void {
    console.log("has value:", withDefault(42, 0));
    console.log("direct:", 10 ?? 20);
    console.log("string fallback:", stringFallback(undefined));
    console.log("string present:", stringFallback("hello"));
}
