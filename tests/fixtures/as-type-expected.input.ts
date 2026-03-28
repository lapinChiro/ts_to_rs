// T3: `as T` expected type propagation
// Tests that `as T` propagates T as expected type to the left-hand expression.

interface Point {
    x: number;
    y: number;
}

// S6: as T should propagate T to object literal
function createPoint(): Point {
    return { x: 10, y: 20 } as Point;
}

// S6: as T with spread
function clonePoint(src: Point): Point {
    return { ...src } as Point;
}
