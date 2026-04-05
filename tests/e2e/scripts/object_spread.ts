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

// Spread AFTER explicit fields: spread overrides the explicit values (rightmost-wins)
function spreadOverridesExplicit(base: Config): Config {
    return { host: "default", port: 0, ...base };
}

// Spread in MIDDLE: spread overrides fields before it, explicit after overrides spread
function spreadMiddleOverride(base: Config): Config {
    return { host: "will-be-overridden", ...base, port: 9999 };
}

// Side-effect preservation: overridden expressions must still be evaluated
function logAndGetPort(label: string): number {
    console.log("side-effect:", label);
    return 9999;
}

function spreadOverridesSideEffect(base: Config): Config {
    return { port: logAndGetPort("overridden"), ...base };
}

// Spread source single-evaluation: fn call as spread source must be called exactly once
function makeConfig(label: string): Config {
    console.log("makeConfig:", label);
    return { host: "from-make", port: 7777 };
}

function spreadFromFnCall(): Config {
    return { ...makeConfig("once") };
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

    // Position ordering: spread overrides explicit fields before it
    const override1 = spreadOverridesExplicit({ host: "real.com", port: 8080 });
    console.log("override1 host:", override1.host);
    console.log("override1 port:", override1.port);

    // Middle spread: host from base, port from explicit after
    const override2 = spreadMiddleOverride({ host: "base.com", port: 3000 });
    console.log("override2 host:", override2.host);
    console.log("override2 port:", override2.port);

    // Side-effect preservation: fn call is overridden by spread but must still execute
    const sideEffectResult = spreadOverridesSideEffect({ host: "final.com", port: 443 });
    console.log("sideEffect host:", sideEffectResult.host);
    console.log("sideEffect port:", sideEffectResult.port);

    // Spread source single-evaluation: makeConfig must print exactly once (not per-field)
    const fromFn = spreadFromFnCall();
    console.log("fromFn host:", fromFn.host);
    console.log("fromFn port:", fromFn.port);  // 7777 from makeConfig, not overridden
}
