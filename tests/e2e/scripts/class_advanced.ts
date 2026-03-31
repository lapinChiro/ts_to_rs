class Greeter {
    greeting: string;

    constructor(greeting: string) {
        this.greeting = greeting;
    }

    greet(name: string): string {
        return this.greeting + ", " + name + "!";
    }
}

class Point {
    constructor(public x: number, public y: number) {}

    distanceTo(other: Point): number {
        const dx: number = this.x - other.x;
        const dy: number = this.y - other.y;
        return Math.sqrt(dx * dx + dy * dy);
    }

    isOrigin(): boolean {
        return this.x === 0 && this.y === 0;
    }
}

function main(): void {
    const g: Greeter = new Greeter("Hello");
    console.log(g.greet("World"));
    console.log(g.greet("Rust"));
    console.log(g.greet("Alice"));

    // Parameter properties
    const p1: Point = new Point(0, 0);
    console.log("p1 x:", p1.x);
    console.log("p1 is origin:", p1.isOrigin());

    const p2: Point = new Point(3, 4);
    console.log("p2 y:", p2.y);
    console.log("p2 is origin:", p2.isOrigin());

    console.log("distance:", p1.distanceTo(p2));
}
