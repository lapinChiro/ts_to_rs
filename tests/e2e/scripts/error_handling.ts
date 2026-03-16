function riskyOperation(x: number): number {
    try {
        if (x < 0) {
            throw new Error("negative");
        }
        console.log("success:", x);
    } catch (e) {
        console.log("caught error");
    }
    return x;
}

function main(): void {
    riskyOperation(5);
    riskyOperation(-1);
}
