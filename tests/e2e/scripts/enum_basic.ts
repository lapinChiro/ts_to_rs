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

enum Direction {
    Up = "UP",
    Down = "DOWN",
    Left = "LEFT",
    Right = "RIGHT",
}

function describeDirection(d: Direction): string {
    switch (d) {
        case Direction.Up:
            return "going up";
        case Direction.Down:
            return "going down";
        case Direction.Left:
            return "going left";
        case Direction.Right:
            return "going right";
        default:
            return "unknown";
    }
}

function main(): void {
    console.log("red:", colorName(Color.Red));
    console.log("green:", colorName(Color.Green));
    console.log("blue:", colorName(Color.Blue));
    console.log("up:", describeDirection(Direction.Up));
    console.log("left:", describeDirection(Direction.Left));
}
