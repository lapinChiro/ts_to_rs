function describe_direction(d: Direction): string {
    if (d == "up") {
        return "going up";
    } else if (d == "down") {
        return "going down";
    } else {
        return "other";
    }
}

function move_to(d: Direction): void {
    console.log("moving:", describe_direction(d));
}

type Direction = "up" | "down" | "left" | "right";

function main(): void {
    const d: Direction = "up";
    console.log(d == "up");
    console.log(d != "down");

    move_to("down");

    const dirs: Direction[] = ["up", "down", "left", "right"];
    for (const dir of dirs) {
        console.log(describe_direction(dir));
    }
}
