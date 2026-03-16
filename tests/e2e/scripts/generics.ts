function identity<T>(value: T): T {
    return value;
}

function wrapValue<T, U>(value: T, label: U): T {
    return value;
}

function main(): void {
    console.log("number:", identity(42));
    console.log("bool:", identity(true));
    console.log("wrap num:", wrapValue(99, "label"));
    console.log("wrap bool:", wrapValue(true, 0));
}
