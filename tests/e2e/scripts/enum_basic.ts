enum Color {
    Red,
    Green,
    Blue,
}

function colorName(c: Color): string {
    switch (c) {
        case Color.Red:
            return "red";
        case Color.Green:
            return "green";
        case Color.Blue:
            return "blue";
        default:
            return "unknown";
    }
}

function main(): void {
    console.log("red:", colorName(Color.Red));
    console.log("green:", colorName(Color.Green));
    console.log("blue:", colorName(Color.Blue));
}
