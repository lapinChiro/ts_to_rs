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
}
