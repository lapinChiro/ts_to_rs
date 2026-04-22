// I-171 Layer 1 Cell B.1.32 (Await operand): `!(await p)`.
// Ideal: tmp-bind on awaited type + falsy predicate.

async function getVal(n: number): Promise<number> {
    return n;
}

async function f(n: number): Promise<string> {
    if (!(await getVal(n))) return "falsy";
    return "truthy";
}

async function main(): Promise<void> {
    console.log(await f(0));  // falsy
    console.log(await f(5));  // truthy
}

main();
