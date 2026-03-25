function handleValue(x: any): string {
    if (x instanceof Date) {
        return x.toISOString();
    }
    if (x instanceof Error) {
        return x.message;
    }
    if (x instanceof RegExp) {
        return x.source;
    }
    return String(x);
}
