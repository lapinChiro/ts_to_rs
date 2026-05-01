// Cell 22: A3 + B1 — top-level Decl::Var with side-effect init + user sync main
// Ideal: rename user main + synthesize fn main with `let p = makePoint(1.0, 2.0); ... __ts_main();`
function main(): void { console.log("from user main"); }
function makePoint(x: number, y: number): { x: number; y: number } { return { x, y }; }
const p = makePoint(1, 2);
console.log(p.x, p.y);
main();
