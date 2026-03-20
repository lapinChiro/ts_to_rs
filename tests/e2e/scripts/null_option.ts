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

    // nullable return value — return value wrapped in Some()
    function findValue(id: number): number | null {
        if (id === 1) {
            return 42;
        }
        return null;
    }
    const v1: number | null = findValue(1);
    const v2: number | null = findValue(99);
    // Check via null comparison (avoids Debug format difference)
    console.log("v1 is null:", v1 === null);
    console.log("v2 is null:", v2 === null);

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
