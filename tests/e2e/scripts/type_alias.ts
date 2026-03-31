interface Point {
    x: number;
    y: number;
}

interface Line {
    start: Point;
    end: Point;
}

function distance(a: Point, b: Point): number {
    const dx: number = a.x - b.x;
    const dy: number = a.y - b.y;
    return Math.sqrt(dx * dx + dy * dy);
}

function main(): void {
    // Interface as struct
    const p: Point = { x: 10, y: 20 };
    console.log("x:", p.x);
    console.log("y:", p.y);

    // Function using interface type
    const d: number = distance({ x: 0, y: 0 }, { x: 3, y: 4 });
    console.log("distance:", d);

    // Nested interface
    const line: Line = { start: { x: 0, y: 0 }, end: { x: 1, y: 1 } };
    console.log("line start x:", line.start.x);
    console.log("line end y:", line.end.y);

    // Array of interfaces
    const points: Point[] = [{ x: 1, y: 2 }, { x: 3, y: 4 }];
    console.log("points length:", points.length);
    console.log("first x:", points[0].x);
}
