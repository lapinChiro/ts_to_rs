interface Config {
  name: string;
  label: string;
}

function greet(): string {
  const s: string = "hello";
  return s;
}

function getNames(): string[] {
  const names: string[] = ["alice", "bob"];
  return names;
}
