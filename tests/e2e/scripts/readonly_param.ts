function sumReadonly(items: readonly number[]): number {
    let total: number = 0;
    for (const item of items) {
        total = total + item;
    }
    return total;
}

function countItems(items: readonly number[]): number {
    return items.length;
}

function main(): void {
    const nums: number[] = [1, 2, 3, 4, 5];
    console.log("sum:", sumReadonly(nums));
    console.log("count:", countItems([10, 20, 30]));
}
