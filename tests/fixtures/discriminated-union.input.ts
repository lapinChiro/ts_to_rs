// Discriminated union types

export type Event = { kind: "click"; x: number } | { kind: "hover"; y: number };

export type Status =
  | { type: "active" }
  | { type: "inactive" }
  | { type: "pending" };

// Discriminated union used in switch
function handleEvent(event: Event): string {
  switch (event.kind) {
    case "click":
      return `clicked at ${event.x}`;
    case "hover":
      return `hovered at ${event.y}`;
  }
}

// Three-variant switch
function describeStatus(status: Status): string {
  switch (status.type) {
    case "active":
      return "active";
    case "inactive":
      return "inactive";
    case "pending":
      return "pending";
  }
}

// Discriminated union with data fields
type Shape =
  | { kind: "circle"; radius: number }
  | { kind: "rect"; width: number; height: number };

function area(shape: Shape): number {
  switch (shape.kind) {
    case "circle":
      return Math.PI * shape.radius * shape.radius;
    case "rect":
      return shape.width * shape.height;
  }
}
