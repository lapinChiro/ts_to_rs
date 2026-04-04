function processNullEq(x: string | null): string {
    if (x === null) {
        return "was null";
    } else {
        return "value: " + x;
    }
}

function processNullNeq(x: string | null): string {
    if (x !== null) {
        return "value: " + x;
    } else {
        return "was null";
    }
}

function main(): void {
    console.log(processNullEq(null));
    console.log(processNullEq("hello"));
    console.log(processNullNeq(null));
    console.log(processNullNeq("world"));
}
