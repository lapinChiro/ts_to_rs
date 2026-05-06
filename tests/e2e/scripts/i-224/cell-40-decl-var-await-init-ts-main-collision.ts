// Cell 40: A3 + B4 + C1 — Decl::Var with await init + `__ts_main` collision + top-level await
// Spec: A3 = Decl::Var with await init, B4 = `__ts_main` collision, C1 = top-await
// Ideal Rust: Tier 2 honest error reclassify "`__ts_main` is reserved for transpiler-internal use;
//   user must rename" (INV-5 collision priority arm 先行 reject、cell 9 と同 wording)
// Empirical (TS, ESM mode): user `function __ts_main` + await + call execute in source order
function __ts_main(): void { console.log("user collision __ts_main"); }
const value = await Promise.resolve(44);
__ts_main();
console.log("got", value);
