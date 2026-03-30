// Private method and property expected type propagation
// Verifies that TypeResolver visits PrivateMethod/PrivateProp bodies

interface Config {
    host: string;
    port: number;
}

class Server {
    name: string;

    constructor(name: string) {
        this.name = name;
    }

    // Private method with return type annotation
    // TypeResolver should propagate Config as expected type for the return object
    private getDefaults(): Config {
        return { host: "localhost", port: 8080 };
    }
}
