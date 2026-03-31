function safeDiv(a: number, b: number): number | null {
    if (b === 0) {
        return null;
    }
    return a / b;
}

function firstPositive(nums: number[]): number | null {
    for (const n of nums) {
        if (n > 0) {
            return n;
        }
    }
    return null;
}

function findByName(names: string[], target: string): string | null {
    for (const name of names) {
        if (name === target) {
            return name;
        }
    }
    return null;
}

function main(): void {
    // Number return or null
    const result1: number | null = safeDiv(10, 2);
    if (result1 !== null) {
        console.log("div:", result1);
    }
    const result2: number | null = safeDiv(10, 0);
    if (result2 === null) {
        console.log("divZero: null");
    }

    // Array search returning element
    const nums1: number[] = [-1, -2, 3, 4];
    const pos: number | null = firstPositive(nums1);
    if (pos !== null) {
        console.log("firstPos:", pos);
    }
    const nums2: number[] = [-1, -2, -3];
    const noPos: number | null = firstPositive(nums2);
    if (noPos === null) {
        console.log("noPos: null");
    }

    // String search returning element
    const names1: string[] = ["alice", "bob", "carol"];
    const found: string | null = findByName(names1, "bob");
    if (found !== null) {
        console.log("found:", found);
    }
    const names2: string[] = ["alice", "bob", "carol"];
    const notFound: string | null = findByName(names2, "dave");
    if (notFound === null) {
        console.log("notFound: null");
    }
}
