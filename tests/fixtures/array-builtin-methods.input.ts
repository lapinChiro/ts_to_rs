function processNumbers(nums: number[]): number[] {
    return nums.map(x => x * 2).filter(x => x > 4);
}

// Without type annotation — relies on TypeRegistry for Array.map return type
function getFirstPositive(nums: number[]): number | undefined {
    const doubled = nums.map(x => x * 2);
    return doubled.find(x => x > 0);
}
