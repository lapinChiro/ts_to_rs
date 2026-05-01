// Cell 27-b: A5 + B0 — top-level Stmt::Debugger
// Ideal: Tier 2 honest error reclassify "`debugger` statement has no Rust equivalent" (本 PRD で wording 確定)
// Current: `_ =>` arm of transform_module_item で UnsupportedSyntaxError (一般 wording)
debugger;
console.log("after debugger");
