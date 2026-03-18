function sum(...nums: number[]): number {
    let total: number = 0;
    for (const n of nums) {
        total += n;
    }
    return total;
}

function countArgs(...args: number[]): number {
    return args.length;
}

function main(): void {
    // Basic rest param call
    console.log("sum:", sum(1, 2, 3));

    // No rest args
    console.log("sum_empty:", sum());

    // More args
    console.log("sum5:", sum(1, 2, 3, 4, 5));

    // Count args
    console.log("count:", countArgs(10, 20, 30, 40));

    // Spread call
    const arr: number[] = [10, 20, 30];
    console.log("sum_spread:", sum(...arr));
}
