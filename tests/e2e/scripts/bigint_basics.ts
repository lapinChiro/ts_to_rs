function main(): void {
    const x: bigint = 5n;
    const y: bigint = 3n;

    // BigInt comparison (avoids printing BigInt directly which differs between TS/Rust)
    if (x > y) {
        console.log("x > y");
    }
    if (x + y === 8n) {
        console.log("sum is 8");
    }
    if (x - y === 2n) {
        console.log("diff is 2");
    }
}
