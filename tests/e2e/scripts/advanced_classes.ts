class Temperature {
    celsius: number;
    constructor(celsius: number) {
        this.celsius = celsius;
    }
    toFahrenheit(): number {
        return this.celsius * 9 / 5 + 32;
    }
    isBoiling(): boolean {
        return this.celsius >= 100;
    }
    isFreezing(): boolean {
        return this.celsius <= 0;
    }
}

class Box {
    width: number;
    height: number;
    depth: number;
    constructor(width: number, height: number, depth: number) {
        this.width = width;
        this.height = height;
        this.depth = depth;
    }
    volume(): number {
        return this.width * this.height * this.depth;
    }
    surfaceArea(): number {
        return 2 * (this.width * this.height + this.height * this.depth + this.width * this.depth);
    }
}

function main(): void {
    const t = new Temperature(100);
    console.log("celsius:", t.celsius);
    console.log("fahrenheit:", t.toFahrenheit());
    console.log("boiling:", t.isBoiling());
    console.log("freezing:", t.isFreezing());

    const t2 = new Temperature(-5);
    console.log("t2 freezing:", t2.isFreezing());
    console.log("t2 boiling:", t2.isBoiling());

    const b = new Box(2, 3, 4);
    console.log("volume:", b.volume());
    console.log("surface:", b.surfaceArea());
}
