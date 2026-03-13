interface Point {
    x: number;
    y: number;
}

function getCoords(p: Point): number {
    const { x, y } = p;
    return x + y;
}

function getMutableCoords(p: Point): number {
    let { x, y } = p;
    x = x + 1;
    return x + y;
}

function getRenamedCoord(p: Point): number {
    const { x: posX } = p;
    return posX;
}
