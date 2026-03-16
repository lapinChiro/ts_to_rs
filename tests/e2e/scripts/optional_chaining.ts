function safeLength(s: string | undefined): number {
    return s?.length ?? -1;
}

function withDefault(value: number | undefined, fallback: number): number {
    return value ?? fallback;
}

function main(): void {
    console.log("has value:", safeLength("hello"));
    console.log("with value:", withDefault(42, 0));
    console.log("with fallback:", withDefault(undefined, 99));
}
