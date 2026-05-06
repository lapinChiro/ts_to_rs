// Cell 79: A6 + B4 + C0 — mixed top-level + `__ts_main` collision + no top-level await
// Spec: A6 = mixed, B4 = collision, C0 = no top-await
// Ideal Rust: Tier 2 honest error reclassify (INV-5 collision priority arm 先行 reject、cell 9 と同 wording)
// Empirical (TS, ESM mode): user `function __ts_main` valid identifier, executes in source order
function __ts_main(): void { console.log("user collision __ts_main"); }
const LIT_VAL = 100;
function compute(): number { return 42; }
const n = compute();
__ts_main();
console.log("got", LIT_VAL, n);
