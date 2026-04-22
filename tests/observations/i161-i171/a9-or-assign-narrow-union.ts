// O-6 observation: `x ||= y` on synthetic union inside narrow scope.
let x: number | string | null = 0;
if (x !== null) {
    // x narrowed to number | string
    x ||= "fallback";  // x=0 → falsy → assign "fallback"
    console.log(x);
}

let y: number | string | null = "hi";
if (y !== null) {
    y ||= "fallback";  // y="hi" → truthy → no assign
    console.log(y);
}

let z: number | string | null = "";
if (z !== null) {
    z ||= "fallback";  // z="" → falsy (empty string) → assign
    console.log(z);
}
