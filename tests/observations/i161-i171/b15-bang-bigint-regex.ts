// B.1.11 / B.1.12 / B.1.13 observation: `!` on BigInt / Regex literals.
// JS: `!0n` = true; `!1n` = false; `!/pattern/` = false (Regex always truthy).
console.log(!0n);        // true
console.log(!1n);        // false
console.log(!/abc/);     // false
console.log(!/pattern/g); // false

function f(x: bigint): boolean { return !x; }
console.log(f(0n));  // true
console.log(f(7n));  // false
