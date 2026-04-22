// A7: `x &&= y` inside narrow scope with synthetic union (number | string).
let x: number | string | null = 5;
if (x !== null) {
    // x narrowed to number | string
    x &&= "hello";  // RHS is string, final x: string
    console.log(x);
}
