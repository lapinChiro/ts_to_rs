class Point {
    x: number;
    y: number;
    constructor(x: number, y: number) {
        this.x = x;
        this.y = y;
    }
    distanceTo(other: Point): number {
        const dx: number = this.x - other.x;
        const dy: number = this.y - other.y;
        return Math.sqrt(dx * dx + dy * dy);
    }
}

function main(): void {
    const p1 = new Point(0, 0);
    const p2 = new Point(3, 4);
    console.log("p1.x:", p1.x);
    console.log("p1.y:", p1.y);
    console.log("distance:", p1.distanceTo(p2));
}
