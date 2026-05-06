// Cell 41: A4 + B0 + C0 — top-level Stmt::If (control-flow at top-level) + no user main
// Spec: A4 = control-flow at top-level (If/For/While/Try/Switch), B0 = no user main, C0 = no top-await
// Ideal Rust: Tier 2 honest error reclassify with improved wording =
//   `UnsupportedSyntaxError::new("ControlFlow at top-level requires fn main wrapping; lift to a named
//    function or use I-203 future expansion", span)` (本 PRD scope 内 wording 改善のみ、Tier 1 化は別 PRD I-203)
// Empirical (TS, ESM mode): TS spec 上 control-flow at top-level は valid module body item、
//   tsx で実行可能 (= TS module-load semantic では control-flow の condition 評価 + body 実行が module load 時に発生)
const x = 7;
if (x > 5) {
    console.log("control-flow ran:", x);
}
