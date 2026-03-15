// Basic switch with break
function getLabel(x: number): string {
    switch (x) {
        case 1:
            return "one";
        case 2:
            return "two";
        default:
            return "other";
    }
}

// Empty fall-through (pattern merging)
function classify(status: string): string {
    switch (status) {
        case "active":
        case "enabled":
            return "on";
        case "inactive":
        case "disabled":
            return "off";
        default:
            return "unknown";
    }
}
