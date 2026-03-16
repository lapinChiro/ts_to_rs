interface Point {
    x: number;
    y: number;
}

function sumCoords(p: Point): number {
    const { x, y } = p;
    return x + y;
}

function main(): void {
    const p: Point = { x: 3, y: 4 };
    console.log("sum:", sumCoords(p));

    // array destructuring
    const arr: number[] = [10, 20, 30];
    const [a, b] = arr;
    console.log("first:", a);
    console.log("second:", b);
}
