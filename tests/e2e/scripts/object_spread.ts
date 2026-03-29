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

// Multiple spreads: later spread overrides earlier
function multiSpread(a: Config, b: Config): Config {
    return { ...a, ...b };
}

function main(): void {
    const base: Config = { host: "example.com", port: 443 };

    const result = withDefaults(base);
    console.log("host:", result.host);
    console.log("port:", result.port);

    extended({ host: "test.com", port: 8080 });

    // Multiple spread: b should override a
    const a: Config = { host: "a.com", port: 80 };
    const b: Config = { host: "b.com", port: 443 };
    const merged = multiSpread(a, b);
    console.log("merged host:", merged.host);
    console.log("merged port:", merged.port);
}
