// String literal union types

type Direction = "up" | "down" | "left" | "right";

type Status = "active" | "inactive";

// Numeric literal union
type HttpCode = 200 | 404 | 500;

// In function parameter
function move(d: Direction): string {
  return `Moving ${d}`;
}

// In function return type
function getStatus(active: boolean): Status {
  return active ? "active" : "inactive";
}

// As interface field type
interface Route {
  path: string;
  method: "GET" | "POST" | "PUT" | "DELETE";
}
