// Cell: I-025 implicit None — for loop early return on Option return
function findPositive(nums: number[]): number | undefined {
    for (const n of nums) {
        if (n > 0) {
            return n;
        }
    }
}

function main(): void {
    const a: number | undefined = findPositive([1, 2, 3]);
    const b: number | undefined = findPositive([-1, -2]);
    const sa: string = a !== undefined ? a.toString() : "none";
    const sb: string = b !== undefined ? b.toString() : "none";
    console.log("implicit-none-for:" + sa + "," + sb);
}
