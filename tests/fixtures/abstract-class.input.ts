// Abstract class with abstract and concrete methods
abstract class Shape {
    abstract area(): number;
    describe(): string { return "I am a shape"; }
}

// Concrete class extending abstract class
class Circle extends Shape {
    radius: number;
    constructor(radius: number) {
        this.radius = radius;
    }
    area(): number { return 3.14 * this.radius * this.radius; }
}
