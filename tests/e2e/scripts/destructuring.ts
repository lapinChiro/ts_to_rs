interface Point {
    x: number;
    y: number;
}

// I-325: object destructuring with optional string default
interface Options {
    width: number;
    color?: string;
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

    // object destructuring with optional string default (I-325)
    const opts: Options = { width: 100 };
    const { color = "black" } = opts;
    console.log("color:", color);

    // function parameter destructuring with optional string default (I-325)
    const opts3: Options = { width: 50 };
    console.log("param color:", getColor(opts3));
    const opts4: Options = { width: 50, color: "blue" };
    console.log("param color:", getColor(opts4));
}

function getColor({ color = "white" }: Options): string {
    return color;
}
