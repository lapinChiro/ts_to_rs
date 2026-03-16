function doWhileSum(): number {
    let sum: number = 0;
    let i: number = 1;
    do {
        sum = sum + i;
        i = i + 1;
    } while (i <= 5);
    return sum;
}

function breakOnThreshold(items: number[]): number {
    let total: number = 0;
    for (const item of items) {
        if (total + item > 10) {
            break;
        }
        total = total + item;
    }
    return total;
}

function skipOdds(items: number[]): number {
    let sum: number = 0;
    for (const item of items) {
        if (item % 2 !== 0) {
            continue;
        }
        sum = sum + item;
    }
    return sum;
}

function main(): void {
    console.log("do-while sum:", doWhileSum());
    console.log("break threshold:", breakOnThreshold([3, 4, 5, 6]));
    console.log("skip odds:", skipOdds([1, 2, 3, 4, 5, 6]));
}
