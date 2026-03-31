function main(): void {
    // C-style for loop
    let sum1: number = 0;
    for (let i: number = 0; i < 5; i++) {
        sum1 = sum1 + i;
    }
    console.log("c-style sum:", sum1);

    // for-of with array
    const items: string[] = ["a", "b", "c"];
    let joined: string = "";
    for (const item of items) {
        joined = joined + item;
    }
    console.log("for-of joined:", joined);

    // for loop with break
    let breakAt: number = 0;
    for (let i: number = 0; i < 100; i++) {
        if (i === 7) {
            breakAt = i;
            break;
        }
    }
    console.log("break at:", breakAt);

    // for loop with continue
    let evenSum: number = 0;
    for (let i: number = 0; i < 10; i++) {
        if (i % 2 !== 0) {
            continue;
        }
        evenSum = evenSum + i;
    }
    console.log("even sum:", evenSum);

    // for loop with step > 1
    let stepSum: number = 0;
    for (let i: number = 0; i < 20; i = i + 3) {
        stepSum = stepSum + i;
    }
    console.log("step3 sum:", stepSum);

    // Nested for loops
    let product: number = 0;
    for (let i: number = 0; i < 3; i++) {
        for (let j: number = 0; j < 3; j++) {
            product = product + 1;
        }
    }
    console.log("nested count:", product);

    // for-of with number array and accumulation
    const nums: number[] = [10, 20, 30, 40];
    let total: number = 0;
    for (const n of nums) {
        total = total + n;
    }
    console.log("for-of total:", total);
}
