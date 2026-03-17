function getPositive(n: number): number {
    return n;
}

function main(): void {
    // Pattern 1: if with numeric assignment (truthy)
    let x: number = 0;
    if (x = getPositive(5)) {
        console.log("x is truthy:", x);
    }

    // Pattern 2: if with numeric assignment (falsy)
    let y: number = 0;
    if (y = getPositive(0)) {
        console.log("y is truthy:", y);
    } else {
        console.log("y is falsy:", y);
    }

    // Pattern 3: if with comparison containing assignment
    let z: number = 0;
    if ((z = getPositive(10)) > 5) {
        console.log("z > 5:", z);
    }

    // Pattern 4: while with numeric assignment
    let counter: number = 3;
    let val: number = 0;
    while (val = counter) {
        console.log("val:", val);
        counter = counter - 1;
    }
}
