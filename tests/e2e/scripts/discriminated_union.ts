type Shape = { kind: "circle"; radius: number } | { kind: "square"; side: number };

function describe_kind(s: Shape): void {
    switch (s.kind) {
        case "circle":
            console.log("it is a circle");
            break;
        case "square":
            console.log("it is a square");
            break;
    }
}

function main(): void {
    const c1: Shape = { kind: "circle", radius: 5 };
    const c2: Shape = { kind: "circle", radius: 5 };
    const sq1: Shape = { kind: "square", side: 3 };
    const sq2: Shape = { kind: "square", side: 3 };

    describe_kind(c1);
    describe_kind(sq1);

    console.log("kind:", c2.kind);
    console.log("kind:", sq2.kind);

    const c3: Shape = { kind: "circle", radius: 5 };
    const sq3: Shape = { kind: "square", side: 3 };
    console.log(c3.kind == "circle");
    console.log(sq3.kind != "circle");
}
