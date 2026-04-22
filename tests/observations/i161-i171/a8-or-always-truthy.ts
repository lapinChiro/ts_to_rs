// O-8 / O-12d observation: `||=` on always-truthy LHS.
// Ideal: const-fold to no-op (empty stmt) because always-truthy never triggers ||= assign.
interface P { a: number }

let p: P = { a: 1 };
p ||= { a: 2 };  // no-op (p was truthy)
console.log(p.a);  // 1

let arr: number[] = [10];
arr ||= [99];
console.log(arr[0]);  // 10
