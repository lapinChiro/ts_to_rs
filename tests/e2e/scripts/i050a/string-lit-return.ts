// Cell 2: String × StringLit × return
function getVal(): any {
    return "hello";
}

function acceptAny(v: any): string {
    return "ok";
}

function main(): void {
    const x: any = getVal();
    console.log("string-lit-return:" + acceptAny(x));
}
