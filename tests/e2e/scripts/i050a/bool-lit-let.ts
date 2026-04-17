// Cell 9: Bool × BoolLit × let-init
function acceptAny(v: any): string {
    return "ok";
}

function main(): void {
    const x: any = true;
    console.log("bool-lit-let:" + acceptAny(x));
}
