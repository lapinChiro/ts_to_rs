class Point {
    constructor(public x: number, public y: number) {}
    distanceTo(other: Point): number {
        const dx: number = this.x - other.x;
        const dy: number = this.y - other.y;
        return Math.sqrt(dx * dx + dy * dy);
    }
}

class Config {
    constructor(
        public name: number,
        private value: number,
        public enabled: boolean
    ) {}
    sum(): number {
        return this.name + this.value;
    }
    isEnabled(): boolean {
        return this.enabled;
    }
}

class MixedParams {
    result: number;
    constructor(public label: number, count: number) {
        this.result = label + count;
    }
    getResult(): number {
        return this.result;
    }
    getLabel(): number {
        return this.label;
    }
}

function main(): void {
    const p1 = new Point(0, 0);
    const p2 = new Point(3, 4);
    console.log("p1.x:", p1.x);
    console.log("p1.y:", p1.y);
    console.log("distance:", p1.distanceTo(p2));

    const cfg = new Config(10, 42, true);
    console.log("sum:", cfg.sum());
    console.log("enabled:", cfg.isEnabled());
    console.log("name:", cfg.name);

    const m = new MixedParams(7, 3);
    console.log("result:", m.getResult());
    console.log("label:", m.getLabel());
}
