// Basic parameter property
class Point {
    constructor(public x: number, public y: number) {}
}

// Multiple visibility levels
class Config {
    constructor(
        public name: string,
        private value: number,
        protected level: number
    ) {}
}

// Readonly parameter property
class Immutable {
    constructor(public readonly id: number) {}
}

// Mixed: param properties + regular params + explicit this assignment
class Mixed {
    extra: number;
    constructor(public label: string, count: number) {
        this.extra = count * 2;
    }
}

// Parameter property with default value
class WithDefault {
    constructor(public size: number = 10) {}
}
