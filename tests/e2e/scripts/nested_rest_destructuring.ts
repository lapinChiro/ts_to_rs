interface Inner {
    a: string;
    b: number;
    c: boolean;
}

interface Outer {
    inner: Inner;
}

function processNested({ inner: { a, ...rest } }: Outer): void {
    console.log("a:", a);
    console.log("b:", rest.b);
    console.log("c:", rest.c);
}

function main(): void {
    const data: Outer = { inner: { a: "hello", b: 42, c: true } };
    processNested(data);
}
