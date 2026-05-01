// Cell 31: A1 + B1 with multiple `main()` calls — substitution invariant verify
// Ideal: rename user main + synthesize `fn main() { __ts_main(); __ts_main(); }` (multiple call sites preserved in source order)
function main(): void { console.log("called"); }
main();
main();
