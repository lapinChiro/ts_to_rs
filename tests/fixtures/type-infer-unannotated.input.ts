class Greeter {
    name: string;
    constructor(name: string) {
        this.name = name;
    }
    greet(msg: string): string {
        return `${this.name}: ${msg}`;
    }
}

function main(): void {
    // const without type annotation — should infer Greeter
    const g = new Greeter("Alice");
    console.log(g.greet("hello"));

    // let without type annotation — should also infer
    let h = new Greeter("Bob");
    console.log(h.greet("world"));
}
