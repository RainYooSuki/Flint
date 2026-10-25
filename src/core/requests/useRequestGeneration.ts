import { useState } from "react";
import { createRequestGeneration, type RequestGeneration } from "./requestGeneration";

export function useRequestGeneration(): RequestGeneration {
  const [generation] = useState(createRequestGeneration);
  return generation;
}
