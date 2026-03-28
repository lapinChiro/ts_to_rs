// E2E: typeof const patterns — number value type through (typeof X)[keyof typeof X]

const Phase = {
  Stringify: 1,
  BeforeStream: 2,
  Stream: 3,
} as const;

type PhaseValue = (typeof Phase)[keyof typeof Phase];

function describePhase(val: PhaseValue): string {
  if (val === 1) {
    return "stringify";
  } else if (val === 2) {
    return "before-stream";
  } else {
    return "stream";
  }
}

function main(): void {
  console.log(describePhase(1));
  console.log(describePhase(2));
  console.log(describePhase(3));
}
