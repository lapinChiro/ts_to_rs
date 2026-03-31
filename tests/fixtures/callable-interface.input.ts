// Callable interface with single call signature
interface GetValue {
    (key: string): string;
}

// Callable interface with overloaded call signatures
interface GetCookie {
    (c: string): string;
    (c: string, key: string): number;
}

// Interface with construct signature
interface Factory {
    new (config: string): Factory;
    name: string;
}

// Type alias referencing a registered interface
interface Body {
    text: string;
    json: boolean;
}

type BodyCache = Body;

// Callable interface used as variable type annotation
const getValue: GetValue = (key: string): string => {
    return key;
};
