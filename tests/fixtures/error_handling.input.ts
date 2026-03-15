export function validateAge(age: number): string {
    if (age < 0) {
        throw new Error("age must be non-negative");
    }
    return "valid";
}

function riskyOperation(): number {
    throw "not implemented";
}

export function safeDivide(a: number, b: number): number {
    if (b === 0) {
        throw new Error("division by zero");
    }
    return a / b;
}

function withTryCatch(): void {
    try {
        riskyOperation();
    } catch (e) {
        console.error(e);
    }
}

function withTryFinally(): void {
    try {
        riskyOperation();
    } finally {
        console.log("cleanup");
    }
}

function withTryCatchFinally(): void {
    try {
        riskyOperation();
    } catch (e) {
        console.error(e);
    } finally {
        console.log("done");
    }
}

function withThrowInTryCatch(x: number): void {
    try {
        if (x < 0) {
            throw new Error("negative");
        }
        console.log("ok");
    } catch (e) {
        console.error(e);
    }
}

function withBreakInTryCatch(items: number[]): void {
    for (const item of items) {
        try {
            if (item < 0) {
                break;
            }
            riskyOperation();
        } catch (e) {
            console.error(e);
        }
    }
}
