// SDCDF smoke test: return context × f64 type × ident AST shape
// Cell: return-f64-ident
// Expected: TS and Rust stdout match exactly

function double(x: number): number {
    return x * 2;
}

function main(): void {
    console.log(double(21));
}
