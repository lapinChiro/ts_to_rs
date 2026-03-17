class Greeter {
    name: string;
    constructor(name: string) {
        this.name = name;
    }
    greet(msg: string): string {
        return `${this.name}: ${msg}`;
    }
}

function createGreeter(name: string): Greeter {
    return new Greeter(name);
}

function main(): void {
    // Type inferred from new expression
    const g = new Greeter("Alice");
    console.log(g.greet("hello"));

    // Type inferred from function return value
    const h = createGreeter("Bob");
    console.log(h.greet("world"));

    // let declaration
    let k = new Greeter("Charlie");
    console.log(k.greet("hi"));
}
