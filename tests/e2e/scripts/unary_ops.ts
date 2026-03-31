function main(): void {
    // Numeric negation
    const pos: number = 42;
    const neg: number = -pos;
    console.log("negation:", neg);
    console.log("double neg:", -(-5));

    // Logical NOT
    console.log("!true:", !true);
    console.log("!false:", !false);
    const val: boolean = true;
    console.log("!val:", !val);
    console.log("!!val:", !!val);

    // typeof on primitives
    const n: number = 10;
    console.log("typeof number:", typeof n);
    const s: string = "hi";
    console.log("typeof string:", typeof s);

    // Negation in complex expression
    const a: number = 3;
    const b: number = 4;
    console.log("neg expr:", -(a + b));

    // Logical NOT on comparison
    console.log("!(5>3):", !(5 > 3));
    console.log("!(1>9):", !(1 > 9));

    // Negation of negative
    const negVal: number = -10;
    console.log("abs via neg:", -negVal);
}
