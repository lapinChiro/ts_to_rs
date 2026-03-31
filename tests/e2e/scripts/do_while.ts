function main(): void {
    // Basic do-while
    let count: number = 0;
    do {
        count = count + 1;
    } while (count < 5);
    console.log("count:", count);

    // do-while with break
    let sum: number = 0;
    let i: number = 1;
    do {
        sum = sum + i;
        if (sum > 10) {
            break;
        }
        i = i + 1;
    } while (i <= 100);
    console.log("sum:", sum);
    console.log("i at break:", i);

    // do-while executes at least once
    let ran: boolean = false;
    do {
        ran = true;
    } while (false);
    console.log("ran once:", ran);

    // Nested do-while
    let result: number = 0;
    let outer: number = 0;
    do {
        let inner: number = 0;
        do {
            result = result + 1;
            inner = inner + 1;
        } while (inner < 3);
        outer = outer + 1;
    } while (outer < 2);
    console.log("nested result:", result);

    // do-while with complex condition
    let x: number = 100;
    do {
        x = Math.floor(x / 2);
    } while (x > 10);
    console.log("halved:", x);

    // do-while with continue (I-341: continue must check condition, not infinite loop)
    let total: number = 0;
    let j: number = 0;
    do {
        j = j + 1;
        if (j % 2 === 0) {
            continue;
        }
        total = total + j;
    } while (j < 10);
    console.log("odd sum:", total);

    // do-while with continue and early exit condition
    let found: number = -1;
    let k: number = 0;
    do {
        k = k + 1;
        if (k % 3 !== 0) {
            continue;
        }
        if (k > 10) {
            found = k;
            break;
        }
    } while (k < 20);
    console.log("first multiple of 3 > 10:", found);
}
