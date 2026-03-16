function safeLength(s: string | undefined): number {
    return s?.length ?? -1;
}

function main(): void {
    console.log("has value:", safeLength("hello"));
}
