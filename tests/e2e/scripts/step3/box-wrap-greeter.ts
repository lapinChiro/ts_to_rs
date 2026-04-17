// Cell: I-020 Box wrap — closure returning string function (no captures)
function makeUpper(): (s: string) => string {
    return (s: string) => s.toUpperCase();
}

function main(): void {
    const upper = makeUpper();
    console.log("box-wrap-greeter:" + upper("hello"));
}
