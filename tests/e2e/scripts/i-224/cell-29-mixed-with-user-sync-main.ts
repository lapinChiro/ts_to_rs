// Cell 29: A6 + B1 — mixed top-level + user sync main
// Ideal: source order preserve + rename user main + control-flow handling は本 PRD scope 外 (= 本 fixture では control-flow stmt は use しない)
function main(): void { console.log("from user main"); }
function compute(): number { return 42; }
const LIT_VAL = 100;
const n = compute();
console.log(LIT_VAL, n);
main();
