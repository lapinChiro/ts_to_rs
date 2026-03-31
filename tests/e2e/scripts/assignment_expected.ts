interface Settings {
    width: number;
    height: number;
    label: string;
}

function showSettings(s: Settings): void {
    console.log("w:", s.width, "h:", s.height, "l:", s.label);
}

function main(): void {
    // Assignment with expected type from annotation
    const s1: Settings = { width: 100, height: 200, label: "main" };
    showSettings(s1);

    // Assignment to function parameter (expected type from parameter)
    showSettings({ width: 50, height: 75, label: "small" });

    // Numeric assignment (int → float)
    const x: number = 42;
    const y: number = x + 0.5;
    console.log("y:", y);

    // String concatenation with number
    const msg: string = "value=" + x;
    console.log("msg:", msg);
}
