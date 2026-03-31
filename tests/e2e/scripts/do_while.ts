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
}
