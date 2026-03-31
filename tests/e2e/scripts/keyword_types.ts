// Tests void, null, boolean keyword types in various contexts

function logMessage(msg: string): void {
    console.log("log:", msg);
}

function doNothing(): void {
    // void return — implicit unit
}

function isEven(n: number): boolean {
    return n % 2 === 0;
}

function classify(n: number): string {
    if (n > 0) {
        return "positive";
    } else if (n < 0) {
        return "negative";
    }
    return "zero";
}

function main(): void {
    // void return type
    logMessage("hello");
    logMessage("world");
    doNothing();

    // boolean return
    console.log("isEven 4:", isEven(4));
    console.log("isEven 7:", isEven(7));

    // null literal with null check
    const maybeNull: string | null = null;
    if (maybeNull === null) {
        console.log("is null: true");
    }

    const hasValue: string | null = "present";
    if (hasValue !== null) {
        console.log("has value:", hasValue);
    }

    // Multi-branch return
    console.log("classify 5:", classify(5));
    console.log("classify -3:", classify(-3));
    console.log("classify 0:", classify(0));
}
