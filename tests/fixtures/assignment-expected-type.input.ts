// T2: Assignment LHS→RHS expected type propagation
// Tests that the type of the left-hand side of an assignment is propagated
// as the expected type of the right-hand side.

interface Config {
    host: string;
    port: number;
}

interface AppState {
    config: Config;
    name: string;
}

// S5: Member assignment should propagate field type
function updateConfig(state: AppState): void {
    let mutableState = state;
    mutableState.config = { host: "localhost", port: 8080 };
}
