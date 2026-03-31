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

    // Optional values (I-339: should print value, not Some(...))
    const maybeNum: number | undefined = 42;
    console.log("maybe num:", maybeNum);

    const maybeStr: string | undefined = "hello";
    console.log("maybe str:", maybeStr);

    const maybeBool: boolean | undefined = true;
    console.log("maybe bool:", maybeBool);

    const noneVal: number | undefined = undefined;
    console.log("none val:", noneVal);
}
