function main(): void {
    // Multiple const declarations
    const a: number = 1, b: number = 2, c: number = 3;
    console.log("a:", a);
    console.log("b:", b);
    console.log("c:", c);

    // Multiple let declarations
    let x: number = 10, y: number = 20;
    console.log("x:", x);
    console.log("y:", y);

    // Mixed types
    const name: string = "hello", value: number = 42, flag: boolean = true;
    console.log("name:", name);
    console.log("value:", value);
    console.log("flag:", flag);

    // Mutation after multi-decl
    let p: number = 1, q: number = 2;
    p = p + q;
    q = q * 3;
    console.log("p:", p);
    console.log("q:", q);
}
