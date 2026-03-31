function main(): void {
    // Arrow function expression assigned to variable
    const add = (a: number, b: number): number => a + b;
    console.log("add:", add(3, 4));

    // Arrow function with block body
    const multiply = (a: number, b: number): number => {
        const result: number = a * b;
        return result;
    };
    console.log("multiply:", multiply(5, 6));

    // Arrow function with no params
    const getHello = (): string => "hello";
    console.log("getHello:", getHello());

    // Closure capturing outer variable (read-only)
    const base: number = 100;
    const addBase = (x: number): number => x + base;
    console.log("addBase:", addBase(42));

    // Arrow function returning boolean
    const isPositive = (n: number): boolean => n > 0;
    console.log("isPositive 5:", isPositive(5));
    console.log("isPositive -3:", isPositive(-3));

    // Nested arrow functions
    const outer = (x: number): number => {
        const inner = (y: number): number => y * 2;
        return inner(x) + 1;
    };
    console.log("nested:", outer(5));
}
