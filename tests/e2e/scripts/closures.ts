function main(): void {
    const double = (x: number): number => x * 2;
    console.log("double 3:", double(3));
    console.log("double 10:", double(10));

    const add = (a: number, b: number): number => a + b;
    console.log("add 3 4:", add(3, 4));

    const isPositive = (n: number): boolean => n > 0;
    console.log("positive 5:", isPositive(5));
    console.log("positive -1:", isPositive(-1));
}
