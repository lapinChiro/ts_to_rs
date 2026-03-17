function main(): void {
    const x: number = 42;
    const result: string = x + " px";
    console.log(result);

    const y: number = 3.14;
    const msg: string = y + " is pi";
    console.log(msg);

    // String + number (existing behavior)
    const prefix: string = "value: " + x;
    console.log(prefix);
}
