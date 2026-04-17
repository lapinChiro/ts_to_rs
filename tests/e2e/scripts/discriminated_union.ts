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

// I-021 Tpl: DU field access inside a template literal must still resolve
// via the destructured match binding (not raw `s.radius`).
function describe_shape(s: Shape): string {
    switch (s.kind) {
        case "circle":
            return `circle with r=${s.radius}`;
        case "square":
            return `square with s=${s.side}`;
    }
}

// I-021 Tpl: nested contexts — template + binary + call arg all exercising
// the walker's Tpl + Bin + Call paths.
function describe_with_suffix(s: Shape, suffix: string): string {
    switch (s.kind) {
        case "circle":
            return `dim=${Math.abs(s.radius)}${suffix}`;
        case "square":
            return `dim=${Math.abs(s.side)}${suffix}`;
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

    // I-021 Tpl: DU field inside template literal.
    // Use fresh bindings for each call to avoid move-after-use in Rust output
    // (a separate ownership concern tracked by I-048).
    const c6: Shape = { kind: "circle", radius: 1.5 };
    const sq5: Shape = { kind: "square", side: 2.5 };
    console.log(describe_shape(c6));
    console.log(describe_shape(sq5));

    const c7: Shape = { kind: "circle", radius: 1.5 };
    const sq6: Shape = { kind: "square", side: 2.5 };
    console.log(describe_with_suffix(c7, "cm"));
    console.log(describe_with_suffix(sq6, "m"));
}
