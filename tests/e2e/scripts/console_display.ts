enum Status {
    Active = 1,
    Inactive = 0,
}

type Direction = "north" | "south" | "east" | "west";

function main(): void {
    // Numeric enum display
    const s: Status = Status.Active;
    console.log(s);
    console.log(Status.Inactive);

    // String literal union enum display
    const d: Direction = "north";
    console.log(d);

    // Mixed: string label + enum
    console.log("status:", s);
    console.log("direction:", d);
}
