export interface RequestGeneration {
  readonly next: () => number;
  readonly invalidate: () => void;
  readonly isCurrent: (requestId: number) => boolean;
}

export function createRequestGeneration(): RequestGeneration {
  let current = 0;

  return {
    next() {
      current += 1;
      return current;
    },
    invalidate() {
      current += 1;
    },
    isCurrent(requestId: number) {
      return requestId === current;
    }
  };
}
