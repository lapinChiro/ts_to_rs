// SDCDF smoke test: let-init context × String type × string literal AST shape
// Cell: let-init-string-lit
// Expected: TS and Rust stdout match exactly

function main(): void {
    const x: string = "hello";
    console.log(x);
}
