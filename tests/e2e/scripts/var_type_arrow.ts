interface Point {
    x: number;
    y: number;
}

const makePoint: (x: number, y: number) => Point = (x: number, y: number) => {
    return { x, y };
};

interface Info {
    name: string;
    value: number;
}

const createInfo: (n: string, v: number) => Info = (n: string, v: number) => {
    return { name: n, value: v };
};

function main(): void {
    const p: Point = makePoint(3, 4);
    console.log("point x:", p.x);
    console.log("point y:", p.y);

    const info: Info = createInfo("test", 42);
    console.log("info name:", info.name);
    console.log("info value:", info.value);
}
