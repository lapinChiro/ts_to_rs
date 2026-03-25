// Anonymous struct inference: untyped object literals generate inline structs

function testBasic(): void {
    const point = { x: 1, y: 2 };
    console.log(point.x);
}

function testNested(): void {
    const nested = { inner: { a: 10, b: 20 } };
    console.log(nested.inner.a);
}
