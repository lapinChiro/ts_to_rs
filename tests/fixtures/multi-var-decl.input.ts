function multiVarConst(): number {
    const a: number = 1, b: number = 2;
    return a + b;
}

function multiVarLet(): number {
    let x: number = 10, y: number = 20;
    x = x + 1;
    return x + y;
}
