// I-138: Vec index read `arr[0]` in `Option<T>` return / assignment / ternary
// context must map to `arr.get(0).cloned()` so empty Vec returns None instead
// of panicking, matching TS `arr[0] === undefined` semantics.

function firstOrNone(items: string[]): string | undefined {
    return items[0];
}

function firstOrNoneTernary(cond: boolean, items: string[]): string | undefined {
    return cond ? items[0] : undefined;
}

function firstOpt(items: (string | undefined)[]): string | undefined {
    return items[0];
}

function main(): void {
    // return context — Option<String>
    const r1: string | undefined = firstOrNone(["a", "b"]);
    const r2: string | undefined = firstOrNone([]);
    const r3: string | undefined = firstOrNone(["solo"]);
    console.log("r1 is undefined:", r1 === undefined);
    console.log("r2 is undefined:", r2 === undefined);
    console.log("r3 is undefined:", r3 === undefined);

    // assignment context — `const x: string | undefined = arr[0]`
    const arr: string[] = ["x", "y"];
    const empty: string[] = [];
    const a1: string | undefined = arr[0];
    const a2: string | undefined = empty[0];
    console.log("a1 is undefined:", a1 === undefined);
    console.log("a2 is undefined:", a2 === undefined);

    // ternary branch (cons) context
    const t1: string | undefined = firstOrNoneTernary(true, ["ok"]);
    const t2: string | undefined = firstOrNoneTernary(true, []);
    const t3: string | undefined = firstOrNoneTernary(false, ["skipped"]);
    console.log("t1 is undefined:", t1 === undefined);
    console.log("t2 is undefined:", t2 === undefined);
    console.log("t3 is undefined:", t3 === undefined);

    // Vec<Option<T>> context — `.flatten()` must collapse Option<Option<T>>.
    // Empty vec, inner undefined, inner defined — all three cases.
    const o1: string | undefined = firstOpt(["x"]);
    const o2: string | undefined = firstOpt([undefined]);
    const o3: string | undefined = firstOpt([]);
    console.log("o1 is undefined:", o1 === undefined);
    console.log("o2 is undefined:", o2 === undefined);
    console.log("o3 is undefined:", o3 === undefined);
}
