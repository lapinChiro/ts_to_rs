function findTernary(x: boolean): string | null {
    return x ? "found" : null;
}

function findDirect(x: boolean): string | null {
    if (x) {
        return "found";
    }
    return null;
}
