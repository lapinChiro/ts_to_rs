// E2E test for callable interface call dispatch (Phase 10-11)
// Verifies TS execution and converted Rust execution produce identical stdout.
// Covers: single overload, zero-arg, generic, multi-overload divergent return, async.

// --- Single overload callable interface ---
interface GetValue {
    (key: string): string;
}

const getValue: GetValue = (key: string): string => {
    return key;
};

// --- Zero-arg callable interface ---
interface GetDefault {
    (): string;
}

const getDefault: GetDefault = (): string => {
    return "default_value";
};

// --- Generic callable interface ---
interface Transform<T, U> {
    (input: T): U;
}

const toNumber: Transform<string, number> = (input: string): number => {
    return 42;
};

// --- Multi-overload with divergent return types ---
interface GetCookie {
    (c: string): string;
    (c: string, key: string): number;
}

const getCookie: GetCookie = (c: string, key?: string): string | number => {
    if (key !== undefined) {
        return 99;
    }
    return c;
};

// --- Async callable interface ---
interface AsyncFetcher {
    (url: string): Promise<string>;
}

const fetchData: AsyncFetcher = async (url: string): Promise<string> => {
    return url;
};

async function main(): Promise<void> {
    // Single overload call dispatch
    console.log(getValue("hello"));
    console.log(getValue("world"));

    // Zero-arg call dispatch
    console.log(getDefault());

    // Generic callable interface call dispatch
    console.log(toNumber("anything"));

    // Multi-overload arity selection: 1-arg → call_0 (returns string)
    console.log(getCookie("ctx"));

    // Multi-overload arity selection: 2-arg → call_1 (returns number)
    console.log(getCookie("ctx", "session"));

    // Async callable interface call dispatch
    const fetched: string = await fetchData("async_result");
    console.log(fetched);
}
