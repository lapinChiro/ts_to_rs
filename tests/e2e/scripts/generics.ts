function identity<T>(value: T): T {
    return value;
}

function main(): void {
    console.log("number:", identity(42));
    console.log("bool:", identity(true));
}
