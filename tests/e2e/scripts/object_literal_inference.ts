// E2E: Object literal type inference (I-112c)
// Tests that untyped object literals are correctly converted and executable

interface Config {
    host: string;
    port: number;
}

function printConfig(cfg: Config): void {
    console.log("host:", cfg.host);
    console.log("port:", cfg.port);
}

// Pattern 1: Typed variable with object literal (already works)
function testTypedVariable(): void {
    const cfg: Config = { host: "localhost", port: 8080 };
    printConfig(cfg);
}

// Pattern 2: Return type inference from function annotation
function makeConfig(): Config {
    return { host: "example.com", port: 443 };
}

// Pattern 3: Call signature type alias
type Formatter = (input: string) => string;
const upperFormatter: Formatter = (input: string): string => {
    return input.toUpperCase();
};

function main(): void {
    testTypedVariable();

    const cfg2: Config = makeConfig();
    console.log("returned host:", cfg2.host);
    console.log("returned port:", cfg2.port);

    console.log("formatted:", upperFormatter("hello"));
}
