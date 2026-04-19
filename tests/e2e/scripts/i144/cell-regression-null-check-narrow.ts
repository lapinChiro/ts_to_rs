// I-144 regression lock-in: `x !== null` narrows Option<T> to T in the then
// branch; complement (`===`) narrows to None. Basic T3a + complement.
// TS runtime: "v=10", "v=none".

function f(x: number | null): string {
    if (x !== null) {
        return "v=" + x;
    }
    return "v=none";
}

function main(): void {
    console.log(f(10));
    console.log(f(null));
}
