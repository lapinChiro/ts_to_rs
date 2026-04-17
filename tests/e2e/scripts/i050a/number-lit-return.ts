// Cell 6: F64 × NumLit × return
function getVal(): any {
    return 42;
}

function acceptAny(v: any): string {
    return "ok";
}

function main(): void {
    const x: any = getVal();
    console.log("number-lit-return:" + acceptAny(x));
}
