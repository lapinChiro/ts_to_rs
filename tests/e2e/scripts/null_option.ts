function main(): void {
    // null assignment to Option type — should compile (not Some(None))
    const a: number | null = null;

    // null check
    if (a === null) {
        console.log("a is null");
    } else {
        console.log("a has value");
    }

    // undefined assignment to Option type
    const c: string | undefined = undefined;
    if (c === undefined) {
        console.log("c is undefined");
    }

    // nested null
    const arr: (number | null)[] = [1, null, 3];
    let nullCount: number = 0;
    for (const item of arr) {
        if (item === null) {
            nullCount += 1;
        }
    }
    console.log("null count:", nullCount);
}
