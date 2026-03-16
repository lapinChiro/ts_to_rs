function main(): void {
    // while loop
    let count: number = 0;
    let n: number = 1;
    while (n < 100) {
        n = n * 2;
        count = count + 1;
    }
    console.log("doublings to 100:", count);

    // for-of
    const items: number[] = [1, 2, 3, 4, 5];
    let found: number = -1;
    for (const item of items) {
        if (item === 3) {
            found = item;
            break;
        }
    }
    console.log("found:", found);

    // for-range with sum (I-57 fix)
    let sum: number = 0;
    for (let i = 0; i < 5; i++) {
        sum = sum + i;
    }
    console.log("sum 0..5:", sum);
}
