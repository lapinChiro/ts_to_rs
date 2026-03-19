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

    // object destructuring with rename
    const p2: Point = { x: 10, y: 20 };
    const { x: px, y: py } = p2;
    console.log("renamed x:", px);
    console.log("renamed y:", py);

    // Arrow with array destructuring parameter
    const get_key = ([k, v]: [string, number]): string => k;
    console.log("key:", get_key(["x", 42]));

    const get_val = ([k, v]: [string, number]): number => v;
    console.log("val:", get_val(["y", 99]));
}
