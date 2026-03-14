export type Event = { kind: "click", x: number } | { kind: "hover", y: number };

export type Status = { type: "active" } | { type: "inactive" } | { type: "pending" };
