// Cell 1: String × StringLit × let-init
function acceptAny(v: any): string {
    return "ok";
}

function main(): void {
    const x: any = "hello";
    console.log("string-lit-let:" + acceptAny(x));
}
