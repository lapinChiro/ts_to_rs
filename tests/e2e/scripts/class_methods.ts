class Temperature {
    celsius: number;

    constructor(celsius: number) {
        this.celsius = celsius;
    }

    getFahrenheit(): number {
        return this.celsius * 9 / 5 + 32;
    }

    getValue(): number {
        return this.celsius;
    }

    isFreezing(): boolean {
        return this.celsius <= 0;
    }
}

class Rectangle {
    width: number;
    height: number;

    constructor(width: number, height: number) {
        this.width = width;
        this.height = height;
    }

    getArea(): number {
        return this.width * this.height;
    }

    getPerimeter(): number {
        return 2 * (this.width + this.height);
    }

    isSquare(): boolean {
        return this.width === this.height;
    }
}

function main(): void {
    const temp: Temperature = new Temperature(100);
    console.log("celsius:", temp.getValue());
    console.log("fahrenheit:", temp.getFahrenheit());
    console.log("freezing:", temp.isFreezing());

    const cold: Temperature = new Temperature(0);
    console.log("cold celsius:", cold.getValue());
    console.log("cold fahrenheit:", cold.getFahrenheit());
    console.log("cold freezing:", cold.isFreezing());

    const rect: Rectangle = new Rectangle(3, 4);
    console.log("area:", rect.getArea());
    console.log("perimeter:", rect.getPerimeter());
    console.log("is square:", rect.isSquare());

    const sq: Rectangle = new Rectangle(5, 5);
    console.log("sq area:", sq.getArea());
    console.log("sq is square:", sq.isSquare());
}
