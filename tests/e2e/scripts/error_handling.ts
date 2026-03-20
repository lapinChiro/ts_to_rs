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

// try/catch 両方が return する関数 
function safeDivide(a: number, b: number): number {
    try {
        if (b === 0) {
            throw new Error("div by zero");
        }
        return a / b;
    } catch (e) {
        return 0;
    }
}

function main(): void {
    riskyOperation(5);
    riskyOperation(-1);
    console.log("safe divide 10/2:", safeDivide(10, 2));
    console.log("safe divide 10/0:", safeDivide(10, 0));
}
