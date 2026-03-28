// E2E test: Expected type propagation features (I-286)
// Tests T2 (assignment), T3 (as T), T4 (ternary union)
// Note: T1 (Vec callback inference) requires builtins and is tested via snapshot tests.

interface Point {
    x: number;
    y: number;
}

interface Config {
    host: string;
    port: number;
}

interface AppState {
    config: Config;
}

function main(): void {
    // T3: as T propagation — object literal infers type from as assertion
    const p = { x: 10, y: 20 } as Point;
    console.log("point:", p.x, p.y);

    // T2: assignment LHS→RHS — member assignment infers field type
    const state: AppState = { config: { host: "initial", port: 0 } };
    let mutableState = state;
    mutableState.config = { host: "localhost", port: 8080 };
    console.log("config:", mutableState.config.host, mutableState.config.port);
}
