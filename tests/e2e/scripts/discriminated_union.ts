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

function get_dimension(s: Shape): number {
    switch (s.kind) {
        case "circle":
            return s.radius;
        case "square":
            return s.side;
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

    // field access in switch arms
    const c4: Shape = { kind: "circle", radius: 10 };
    const sq4: Shape = { kind: "square", side: 7 };
    console.log("dim:", get_dimension(c4));
    console.log("dim:", get_dimension(sq4));

    // standalone field access
    const c5: Shape = { kind: "circle", radius: 42 };
    console.log("radius:", c5.radius);
}
