// Explicit type arguments in call and new expressions
// Verifies that type_args are used to instantiate type parameters

interface Config {
    host: string;
    port: number;
}

class Container<T> {
    value: T;
    constructor(value: T) {
        this.value = value;
    }
}

function identity<T>(x: T): T {
    return x;
}

// Explicit type args on new expression and function call
function createInstances(): void {
    const c = new Container<Config>({ host: "localhost", port: 8080 });
    const config = identity<Config>({ host: "localhost", port: 8080 });
}
