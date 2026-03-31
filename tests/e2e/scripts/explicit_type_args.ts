function identity<T>(val: T): T {
    return val;
}

function makeArray<T>(a: T, b: T): T[] {
    return [a, b];
}

function pickFirst<A, B>(a: A, b: B): A {
    return a;
}

function main(): void {
    // Explicit type args on identity
    const n: number = identity<number>(42);
    console.log("number:", n);

    const s: string = identity<string>("hello");
    console.log("string:", s);

    // Explicit type args on array-returning function
    const nums: number[] = makeArray<number>(1, 2);
    console.log("nums length:", nums.length);

    // Multiple type parameters
    const first: number = pickFirst<number, string>(99, "ignored");
    console.log("pickFirst:", first);

    // Type args with boolean
    const bools: boolean[] = makeArray<boolean>(true, false);
    console.log("bools length:", bools.length);
}
