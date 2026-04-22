// A-8 / A-12d observation: `&&=` on always-truthy LHS (Named / Array / Tuple).
// Ideal: const-fold to `x = y;` because always-truthy short-circuits to RHS.
interface P { a: number }

let p: P = { a: 1 };
p &&= { a: 2 };
console.log(p.a);  // 2

let arr: number[] = [1, 2, 3];
arr &&= [4, 5];
console.log(arr.length, arr[0]);  // 2 4

let t: [number, string] = [1, "x"];
t &&= [2, "y"];
console.log(t[0], t[1]);  // 2 y
