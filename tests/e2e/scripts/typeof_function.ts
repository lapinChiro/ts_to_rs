// E2E test for typeof type resolution in function parameters.

function double(x: number): number {
    return x * 2;
}

function apply(f: typeof double, value: number): number {
    return f(value);
}

function main(): void {
    console.log("double 5:", double(5));
    console.log("apply 7:", apply(double, 7));
}
