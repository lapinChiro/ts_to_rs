// I-161 Cell A-{Member LHS, &&=}: `obj.field &&= y`.
// Ideal: `if <truthy predicate on obj.field type> { obj.field = y; }`.

function f(): string {
    const obj: { x: number; y: number | null } = { x: 5, y: null };
    obj.x &&= 10;
    obj.y &&= 10;
    return `${obj.x},${obj.y === null ? "null" : obj.y}`;
}

function main(): void {
    console.log(f()); // "10,null"
}
