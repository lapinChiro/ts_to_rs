class Shape {
    sides: number;
    constructor(sides: number) {
        this.sides = sides;
    }
    perimeter(sideLength: number): number {
        return this.sides * sideLength;
    }
}

class Square extends Shape {
    constructor() {
        super(4);
    }
    area(sideLength: number): number {
        return sideLength * sideLength;
    }
}

function main(): void {
    const s = new Square();
    console.log("sides:", s.sides);
    console.log("perimeter:", s.perimeter(5));
    console.log("area:", s.area(5));
}
