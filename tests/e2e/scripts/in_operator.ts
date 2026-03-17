interface Point {
    x: number;
    y: number;
}

function main(): void {
    const p: Point = { x: 1, y: 2 };

    // struct field exists → true
    console.log("x in p:", "x" in p);
    console.log("y in p:", "y" in p);

    // struct field missing → false
    console.log("z in p:", "z" in p);

    // negation
    console.log("not z in p:", !("z" in p));
}
