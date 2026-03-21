function main(): void {
    // String method chain: trim → split → join
    const result1: string = "  hello world  ".trim().split(" ").join("-");
    console.log("chain1:", result1);

    // String chain: toLowerCase → trim
    const result2: string = "  HELLO  ".toLowerCase().trim();
    console.log("chain2:", result2);

    // String chain: split → join
    const result3: string = "a-b-c".split("-").join(" ");
    console.log("chain3:", result3);

    // String chain: toUpperCase → split → join
    const result4: string = "hello world".toUpperCase().split(" ").join("_");
    console.log("chain4:", result4);
}
