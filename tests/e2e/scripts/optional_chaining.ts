function safeLength(s: string | undefined): number {
    return s?.length ?? -1;
}

function withDefault(value: number | undefined, fallback: number): number {
    return value ?? fallback;
}

function safeUpper(s: string): string {
    return s.toUpperCase();
}

function safeUpperOpt(s: string | undefined): number {
    return s?.toUpperCase()?.length ?? -1;
}

function main(): void {
    console.log("has value:", safeLength("hello"));
    console.log("with value:", withDefault(42, 0));
    console.log("with fallback:", withDefault(undefined, 99));

    // I-81: method call name mapping (toUpperCase → to_uppercase)
    console.log("upper hello:", safeUpper("hello"));
    console.log("upper opt:", safeUpperOpt("world"));
    console.log("upper opt undef:", safeUpperOpt(undefined));
}
