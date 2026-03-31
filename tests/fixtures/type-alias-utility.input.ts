// Type alias referencing a registered type (simple ref)
interface Config {
    host: string;
    port: number;
    debug: boolean;
}

type AppConfig = Config;

// Type alias with Partial utility type
type OptionalConfig = Partial<Config>;

// Intersection of type literal and type ref
type ExtendedConfig = Config & { extra: string };

// Intersection of two type literals
type Combined = { a: number } & { b: string };
