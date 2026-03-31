interface Config {
    name: string;
    port: number;
    debug: boolean;
}

function showConfig(c: Config): void {
    console.log("name:", c.name);
    console.log("port:", c.port);
    console.log("debug:", c.debug);
}

interface Labeled {
    label: string;
    value: number;
}

function formatLabel(item: Labeled): string {
    return item.label + "=" + item.value;
}

function main(): void {
    const cfg: Config = { name: "app", port: 8080, debug: true };
    showConfig(cfg);

    const prod: Config = { name: "prod", port: 443, debug: false };
    showConfig(prod);

    const item1: Labeled = { label: "x", value: 10 };
    const item2: Labeled = { label: "y", value: 20 };
    console.log("item1:", formatLabel(item1));
    console.log("item2:", formatLabel(item2));
}
