// Single overload callable interface
interface GetValue {
    (key: string): string;
}

// Multi-overload callable interface
interface GetCookie {
    (c: string): string;
    (c: string, key: string): number;
}

const getValue: GetValue = (key: string): string => {
    return key;
};

const getCookie: GetCookie = (c: string, key?: string): string => {
    return c;
};

// Call site dispatch: single overload
function useSingle(): string {
    return getValue("myKey");
}

// Call site dispatch: multi-overload arity selection
function useMulti(): void {
    const result1 = getCookie("ctx");
    const result2 = getCookie("ctx", "name");
}
