// Cell 23: A3 + B2 — top-level Decl::Var with side-effect init + user async main
// Ideal: rename user async main + #[tokio::main] async fn main with let bindings + substituted main() call
async function main(): Promise<void> { console.log("from async main"); }
function makePoint(x: number, y: number): { x: number; y: number } { return { x, y }; }
const p = makePoint(1, 2);
console.log(p.x, p.y);
main();
