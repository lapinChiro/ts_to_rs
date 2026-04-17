// Cell 5: F64 × NumLit × let-init
function acceptAny(v: any): string {
    return "ok";
}

function main(): void {
    const x: any = 42;
    console.log("number-lit-let:" + acceptAny(x));
}
