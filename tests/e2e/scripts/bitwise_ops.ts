function main(): void {
    const a: number = 0xFF;
    const b: number = 0x0F;

    // Basic bitwise operators
    console.log("and:", a & b);
    console.log("or:", a | b);
    console.log("xor:", a ^ b);
    console.log("shl:", 1 << 4);
    console.log("shr:", 256 >> 2);

    // Compound assignment operators
    let x: number = 0xFF;
    x &= 0x0F;
    console.log("and_assign:", x);

    x = 0x0F;
    x |= 0xF0;
    console.log("or_assign:", x);

    x = 0xFF;
    x ^= 0x0F;
    console.log("xor_assign:", x);

    x = 1;
    x <<= 4;
    console.log("shl_assign:", x);

    x = 256;
    x >>= 2;
    console.log("shr_assign:", x);

    // Nested bitwise
    console.log("nested:", (a & b) | 0xF0);

    // Mixed with arithmetic
    console.log("mixed:", 10 + (a & b));
}
