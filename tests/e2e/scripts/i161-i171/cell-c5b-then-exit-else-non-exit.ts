// I-171 T5 deep-fix lock-in (Spec gap discovered at /check_job deep review,
// 2026-04-24): `if (!x) return; else <non-exit>;` on Option<F64>.
//
// TS narrows `x` to `f64` post-if because the only path reaching post-if is
// the (truthy) else branch. Pre-deep-fix Rust emission used the bare
// `Stmt::Match` ElseBranch shape which left `x: Option<f64>` post-if and
// E0369'd on `x + 1`. Post-deep-fix the EarlyReturnFromExitWithElse shape
// wraps the match in `let x = ...;` and tail-emits the narrowed value.
//
// Ideal Rust:
//   fn f(x: Option<f64>) -> f64 {
//       let x = match x {
//           Some(x) if x != 0.0 && !x.is_nan() => {
//               println!("non-exit else");
//               x  // tail expr feeds the outer let
//           }
//           _ => { return -1.0; }
//       };
//       x + 1.0  // x: f64 here
//   }

function f(x: number | null): number {
    if (!x) {
        return -1;
    } else {
        console.log("non-exit else");
    }
    return x + 1;
}

function main(): void {
    console.log(f(null));   // -1 (then-branch exit)
    console.log(f(0));      // -1 (0 is JS-falsy → then-branch exit)
    console.log(f(5));      // "non-exit else" then 6
}
