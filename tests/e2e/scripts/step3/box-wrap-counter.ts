// Cell: I-020 Box wrap — closure returning pure function
function makeDoubler(): (x: number) => number {
    return (x: number) => x * 2;
}

function main(): void {
    const doubler = makeDoubler();
    console.log("box-wrap-counter:" + doubler(21));
}
