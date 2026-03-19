function main(): void {
    // Simple arrow IIFE
    const x: number = ((n: number): number => n * 2)(21);
    console.log("double:", x);

    // Arrow IIFE with multiple args
    const sum: number = ((a: number, b: number): number => a + b)(10, 20);
    console.log("sum:", sum);

    // Arrow IIFE returning string
    const msg: string = ((): string => "hello iife")();
    console.log("msg:", msg);
}
