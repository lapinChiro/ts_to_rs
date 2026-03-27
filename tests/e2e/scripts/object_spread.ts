interface Config {
    host: string;
    port: number;
}

// Basic object spread: override specific fields
function withDefaults(base: Config): Config {
    return { ...base, host: "localhost" };
}

// Spread with additional explicit fields → anonymous struct
function extended(base: Config): void {
    const result = { ...base, extra: true };
    console.log("host:", result.host);
    console.log("port:", result.port);
    console.log("extra:", result.extra);
}

function main(): void {
    const base: Config = { host: "example.com", port: 443 };

    const result = withDefaults(base);
    console.log("host:", result.host);
    console.log("port:", result.port);

    extended({ host: "test.com", port: 8080 });
}
