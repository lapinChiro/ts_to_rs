// Call signature interface
export interface Handler {
  (req: string): number;
}

// Mixed properties and methods
export interface Service {
  name: string;
  start(): void;
  stop(): boolean;
}
