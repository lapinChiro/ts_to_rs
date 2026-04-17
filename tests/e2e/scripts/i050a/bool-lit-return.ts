// Cell 10: Bool × BoolLit × return
function getVal(): any {
    return true;
}

function acceptAny(v: any): string {
    return "ok";
}

function main(): void {
    const x: any = getVal();
    console.log("bool-lit-return:" + acceptAny(x));
}
