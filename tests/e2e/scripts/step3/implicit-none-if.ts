// Cell: I-025 implicit None — if without else on Option return
function maybeReturn(flag: boolean): string | void {
    if (flag) {
        return "hello";
    }
}

function main(): void {
    const a: string | void = maybeReturn(true);
    const b: string | void = maybeReturn(false);
    const sa: string = a !== undefined ? a : "none";
    const sb: string = b !== undefined ? "some" : "none";
    console.log("implicit-none-if:" + sa + "," + sb);
}
