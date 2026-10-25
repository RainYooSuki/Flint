#!/usr/bin/env node
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const repoRoot = fileURLToPath(new URL("../", import.meta.url));

async function importTranspiledTypeScript(relativePath) {
  const filename = path.join(repoRoot, relativePath);
  const source = await readFile(filename, "utf8");
  const output = ts.transpileModule(source, {
    fileName: filename,
    compilerOptions: {
      module: ts.ModuleKind.ESNext,
      target: ts.ScriptTarget.ES2022,
      isolatedModules: true
    }
  }).outputText;
  const encoded = Buffer.from(output, "utf8").toString("base64");
  return import(`data:text/javascript;base64,${encoded}`);
}

const { parseLoaderKind } = await importTranspiledTypeScript("src/utils.ts");
const { buildContributionRegistry } = await importTranspiledTypeScript("src/core/contributions.ts");
const {
  evaluateAvailability,
  filterContributionRegistry,
  sanitizeAvailabilityReason
} = await importTranspiledTypeScript("src/core/availability.ts");
const { createRequestGeneration } = await importTranspiledTypeScript(
  "src/core/requests/requestGeneration.ts"
);
const { createHostEventSubscriber } = await importTranspiledTypeScript(
  "src/core/events/hostEvents.ts"
);

const CANONICAL_LOADER_KINDS = [
  "fabric",
  "forge",
  "neoforge",
  "quilt",
  "liteloader",
  "optifine"
];

function makeOverlay({
  featureId = "flint.example",
  id = "flint.example.overlay.test",
  order = 10,
  component = () => null,
  visible
} = {}) {
  return {
    id,
    featureId,
    order,
    component,
    visible
  };
}

const TEST_EVENT_NAMES = {
  installProgress: "flint://install-progress",
  loaderInstallProgress: "flint://loader-install-progress",
  modInstallProgress: "flint://mod-install-progress"
};

const WINDOWS_PATH = "C:\\Users\\Flint\\private-windows.txt";
const FORWARD_WINDOWS_PATH = "D:/Games/Flint/private-forward.log";
const UNIX_PATH = "/home/flint/.config/flint/private-unix.json";
const UNC_PATH = "\\\\server\\share\\private-unc.txt";
const RELATIVE_PATH = "saves/world/private-relative.dat";

function deferred() {
  let resolve;
  const promise = new Promise((next) => {
    resolve = next;
  });
  return { promise, resolve };
}

function settleRequest(generation, requestId, outcome) {
  return generation.isCurrent(requestId)
    ? { status: "current", outcome }
    : { status: "stale", outcome };
}

async function completeOutOfOrder(generation, olderId, newerId, outcome) {
  const olderGate = deferred();
  const newerGate = deferred();
  let olderResult;
  let newerResult;

  const olderPromise = olderGate.promise
    .then(() => settleRequest(generation, olderId, outcome))
    .then((result) => {
      olderResult = result;
    });
  const newerPromise = newerGate.promise
    .then(() => settleRequest(generation, newerId, outcome))
    .then((result) => {
      newerResult = result;
    });

  newerGate.resolve();
  await newerPromise;
  olderGate.resolve();
  await olderPromise;

  return { olderResult, newerResult };
}

function applyLoadingCompletion(generation, state, requestId) {
  if (!generation.isCurrent(requestId)) return state;
  return { ...state, loading: false };
}

function makeFeature({
  featureId = "flint.example",
  backendRequires = [],
  overlays = []
} = {}) {
  return {
    id: featureId,
    featureVersion: "1.0.0",
    requires: [],
    backendRequires,
    contributions: {
      pages: [],
      navigation: [],
      settingsSections: [],
      instanceActions: [],
      overlays
    }
  };
}

test("parseLoaderKind accepts only exact canonical loader IDs", () => {
  for (const loaderKind of CANONICAL_LOADER_KINDS) {
    assert.equal(parseLoaderKind(loaderKind), loaderKind);
    assert.equal(parseLoaderKind(`  ${loaderKind.toUpperCase()}  `), loaderKind);
  }
});

test("parseLoaderKind rejects aliases, substrings, and non-loader values", () => {
  const rejected = [
    "",
    "   ",
    "vanilla",
    "原版",
    "unknown",
    "fabric-loader",
    "forge-1.20.1",
    "myforge",
    "neoforged",
    "quilted",
    "liteloader2",
    "optifine_preview",
    "optifine-hd"
  ];
  for (const value of rejected) {
    assert.equal(parseLoaderKind(value), null, `${JSON.stringify(value)} must not resolve to a loader`);
  }
});

test("buildContributionRegistry orders and freezes overlay contributions", () => {
  const late = makeOverlay({
    id: "flint.example.overlay.late",
    order: 20,
    visible: (context) => context.accountDialogOpen
  });
  const early = makeOverlay({
    id: "flint.example.overlay.early",
    order: 10
  });
  const registry = buildContributionRegistry([makeFeature({ overlays: [late, early] })]);

  assert.deepEqual(
    registry.overlays.map((overlay) => overlay.id),
    ["flint.example.overlay.early", "flint.example.overlay.late"]
  );
  assert.equal(Object.isFrozen(registry.overlays), true);
  assert.equal(Object.isFrozen(registry.overlays[0]), true);
  assert.equal(registry.overlays[0].visible, undefined);
  assert.equal(registry.overlays[1].visible({ accountDialogOpen: true }), true);
  assert.equal(registry.overlays[1].visible({ accountDialogOpen: false }), false);
});

test("overlay validation rejects missing arrays, invalid functions, and duplicate IDs", () => {
  const missingOverlays = makeFeature();
  delete missingOverlays.contributions.overlays;
  assert.throws(
    () => buildContributionRegistry([missingOverlays]),
    /contributions\.overlays must be an array/
  );

  assert.throws(
    () => buildContributionRegistry([
      makeFeature({ overlays: [makeOverlay({ component: null })] })
    ]),
    /component must be a function reference/
  );

  assert.throws(
    () => buildContributionRegistry([
      makeFeature({ overlays: [makeOverlay({ visible: "not-a-function" })] })
    ]),
    /visible selector must be a function reference/
  );

  assert.throws(
    () => buildContributionRegistry([
      makeFeature({ overlays: [makeOverlay(), makeOverlay()] })
    ]),
    /duplicated across frontend slots/
  );
});

test("availability filtering retains overlays only for active features", () => {
  const readyFeature = makeFeature({
    featureId: "flint.ready",
    backendRequires: ["flint.backend.ready"],
    overlays: [
      makeOverlay({
        featureId: "flint.ready",
        id: "flint.ready.overlay.ready"
      })
    ]
  });
  const unavailableFeature = makeFeature({
    featureId: "flint.unavailable",
    backendRequires: ["flint.backend.unavailable"],
    overlays: [
      makeOverlay({
        featureId: "flint.unavailable",
        id: "flint.unavailable.overlay.unavailable"
      })
    ]
  });
  const registry = buildContributionRegistry([readyFeature, unavailableFeature]);
  const evaluation = evaluateAvailability({
    revision: 1,
    hostState: "ready",
    lastFailure: null,
    features: [
      { featureId: "flint.backend.ready", state: "ready", reason: null },
      { featureId: "flint.backend.unavailable", state: "failed", reason: "not ready" }
    ]
  });

  assert.equal(evaluation.valid, true);
  const filtered = filterContributionRegistry(registry, evaluation);
  assert.deepEqual(
    filtered.overlays.map((overlay) => overlay.id),
    ["flint.ready.overlay.ready"]
  );
  assert.equal(Object.isFrozen(filtered.overlays), true);
});

test("request generation issues monotonic IDs and tracks the newest request", () => {
  const generation = createRequestGeneration();
  const first = generation.next();
  const second = generation.next();

  assert.equal(first, 1);
  assert.equal(second, 2);
  assert.equal(generation.isCurrent(first), false);
  assert.equal(generation.isCurrent(second), true);
});

test("out-of-order success applies only the newest request", async () => {
  const generation = createRequestGeneration();
  const olderId = generation.next();
  const newerId = generation.next();
  const { olderResult, newerResult } = await completeOutOfOrder(
    generation,
    olderId,
    newerId,
    "success"
  );

  assert.deepEqual(newerResult, { status: "current", outcome: "success" });
  assert.deepEqual(olderResult, { status: "stale", outcome: "success" });
  assert.equal(generation.isCurrent(newerId), true);
  assert.equal(generation.isCurrent(olderId), false);
});

test("out-of-order error is stale while the newest error is current", async () => {
  const generation = createRequestGeneration();
  const olderId = generation.next();
  const newerId = generation.next();
  const { olderResult, newerResult } = await completeOutOfOrder(
    generation,
    olderId,
    newerId,
    "error"
  );

  assert.deepEqual(newerResult, { status: "current", outcome: "error" });
  assert.deepEqual(olderResult, { status: "stale", outcome: "error" });
  assert.equal(generation.isCurrent(newerId), true);
  assert.equal(generation.isCurrent(olderId), false);
});

test("invalidation makes prior requests stale and later requests current", () => {
  const generation = createRequestGeneration();
  const first = generation.next();
  assert.equal(generation.isCurrent(first), true);

  generation.invalidate();
  assert.equal(generation.isCurrent(first), false);

  const later = generation.next();
  assert.equal(later, 3);
  assert.equal(generation.isCurrent(first), false);
  assert.equal(generation.isCurrent(later), true);
});

test("loading completion is owned by the newest request", () => {
  const generation = createRequestGeneration();
  const olderId = generation.next();
  const newerId = generation.next();
  let state = { loading: true, owner: olderId };

  state = { ...state, loading: true, owner: newerId };
  state = applyLoadingCompletion(generation, state, olderId);
  assert.deepEqual(state, { loading: true, owner: newerId });

  state = applyLoadingCompletion(generation, state, newerId);
  assert.deepEqual(state, { loading: false, owner: newerId });

  generation.invalidate();
  assert.equal(generation.isCurrent(olderId), false);
  assert.equal(generation.isCurrent(newerId), false);
});

test("host event subscriber cleans up a late registration idempotently", async () => {
  const registration = deferred();
  let listener;
  let subscribedEventName;
  const events = createHostEventSubscriber(TEST_EVENT_NAMES, (eventName, handler) => {
    subscribedEventName = eventName;
    listener = handler;
    return registration.promise;
  });
  const received = [];
  const unsubscribe = events.subscribe("installProgress", (payload) => received.push(payload));

  assert.equal(subscribedEventName, TEST_EVENT_NAMES.installProgress);
  assert.equal(typeof listener, "function");

  unsubscribe();
  unsubscribe();

  let removeCount = 0;
  registration.resolve(() => {
    removeCount += 1;
  });
  await registration.promise;
  await Promise.resolve();

  assert.equal(removeCount, 1);
  listener({ phase: "after-unsubscribe" });
  assert.deepEqual(received, []);

  unsubscribe();
  assert.equal(removeCount, 1);
});

test("host event subscriber removes a resolved listener once and stops delivery", async () => {
  let listener;
  let subscribedEventName;
  let removeCount = 0;
  const events = createHostEventSubscriber(TEST_EVENT_NAMES, (eventName, handler) => {
    subscribedEventName = eventName;
    listener = handler;
    return Promise.resolve(() => {
      removeCount += 1;
    });
  });
  const received = [];
  const unsubscribe = events.subscribe("modInstallProgress", (payload) => received.push(payload));

  assert.equal(subscribedEventName, TEST_EVENT_NAMES.modInstallProgress);
  await Promise.resolve();
  await Promise.resolve();

  listener({ phase: "while-open" });
  assert.deepEqual(received, [{ phase: "while-open" }]);

  unsubscribe();
  unsubscribe();
  assert.equal(removeCount, 1);

  listener({ phase: "after-unsubscribe" });
  assert.deepEqual(received, [{ phase: "while-open" }]);
});

test("host event subscriber reports registration failures while open", async () => {
  const failure = new Error("subscription failed");
  const events = createHostEventSubscriber(TEST_EVENT_NAMES, () => Promise.reject(failure));
  let reportedError;
  events.subscribe("loaderInstallProgress", () => undefined, (error) => {
    reportedError = error;
  });

  await Promise.resolve();
  await Promise.resolve();
  assert.equal(reportedError, failure);

  const syncEvents = createHostEventSubscriber(TEST_EVENT_NAMES, () => {
    throw failure;
  });
  let syncError;
  syncEvents.subscribe("installProgress", () => undefined, (error) => {
    syncError = error;
  });
  assert.equal(syncError, failure);
});

test("availability sanitizer redacts token labels and authorization values", () => {
  const secretValues = ["alpha-secret", "beta-secret", "gamma-secret", "delta-secret"];
  const sanitized = sanitizeAvailabilityReason(
    `access_token="alpha-secret" refresh-token='beta-secret' api_key=gamma-secret authorization=delta-secret`
  );

  assert.ok(sanitized);
  for (const secret of secretValues) {
    assert.equal(sanitized.includes(secret), false, `${secret} must be redacted`);
  }
  assert.match(sanitized, /access_token=\[redacted\]/);
  assert.match(sanitized, /refresh-token=\[redacted\]/);
  assert.match(sanitized, /api_key=\[redacted\]/);
  assert.match(sanitized, /authorization=\[redacted\]/);
});

test("availability sanitizer redacts Bearer credentials", () => {
  const sanitized = sanitizeAvailabilityReason(
    "request failed: Bearer eyJhbGciOi.payload.signature-123"
  );

  assert.ok(sanitized);
  assert.equal(sanitized.includes("eyJhbGciOi"), false);
  assert.match(sanitized, /Bearer \[redacted\]/);
});

test("availability sanitizer redacts structured and bare provider IDs", () => {
  const sanitized = sanitizeAvailabilityReason(
    "provider=curseforge.internal provider id 'modrinth.official' via Fabric and OptiFine"
  );

  assert.ok(sanitized);
  for (const providerId of ["curseforge", "modrinth", "fabric", "optifine"]) {
    assert.equal(
      sanitized.toLowerCase().includes(providerId),
      false,
      `${providerId} must be redacted`
    );
  }
  assert.match(sanitized, /\[provider\]/);
});

test("availability sanitizer redacts HTTP URLs", () => {
  const sanitized = sanitizeAvailabilityReason(
    "fetch https://api.example.com/v1/items?access_token=url-secret and http://localhost:1420/private"
  );

  assert.ok(sanitized);
  for (const value of ["api.example.com", "url-secret", "localhost"]) {
    assert.equal(sanitized.includes(value), false, `${value} must be redacted`);
  }
  assert.equal((sanitized.match(/\[url\]/g) ?? []).length, 2);
});

test("availability sanitizer redacts Windows, Unix, UNC, and relative paths", () => {
  const paths = [
    WINDOWS_PATH,
    FORWARD_WINDOWS_PATH,
    UNIX_PATH,
    UNC_PATH,
    RELATIVE_PATH
  ];

  for (const value of paths) {
    const sanitized = sanitizeAvailabilityReason(`open failed: ${value}`);
    assert.ok(sanitized);
    assert.equal(sanitized.includes(value), false, `${value} must be redacted`);
    assert.equal(sanitized.includes("private-"), false);
    assert.match(sanitized, /\[path\]/);
  }
});

test("availability sanitizer normalizes controls, trims, and bounds output", () => {
  const cleaned = sanitizeAvailabilityReason("\u0000 \t alpha\u0007\n beta \u001f ");

  assert.equal(cleaned, "alpha beta");
  assert.equal(/[\u0000-\u001f\u007f]/.test(cleaned), false);
  assert.equal(cleaned, cleaned.trim());
  assert.equal(sanitizeAvailabilityReason(" \u0000 \u0007 \n\t "), null);
  assert.equal(sanitizeAvailabilityReason(""), null);
  assert.equal(sanitizeAvailabilityReason(null), null);

  const bounded = sanitizeAvailabilityReason("x".repeat(500));
  assert.ok(bounded);
  assert.ok(bounded.length > 0 && bounded.length <= 240);
  assert.equal(bounded.endsWith("..."), true);
});

test("availability sanitizer leaves no fixture secrets or paths", () => {
  const reason = [
    `access_token="alpha-secret"`,
    `refresh-token='beta-secret'`,
    "api_key=gamma-secret",
    "authorization=delta-secret",
    "Bearer bearer-secret",
    "provider=curseforge.internal",
    "provider id 'modrinth.official'",
    "via Fabric",
    "fetch https://api.example.com/private?access_token=url-secret",
    WINDOWS_PATH,
    FORWARD_WINDOWS_PATH,
    UNIX_PATH,
    UNC_PATH,
    RELATIVE_PATH
  ].join(" ");
  const sanitized = sanitizeAvailabilityReason(reason);

  assert.ok(sanitized);
  const forbidden = [
    "alpha-secret",
    "beta-secret",
    "gamma-secret",
    "delta-secret",
    "bearer-secret",
    "curseforge.internal",
    "modrinth.official",
    "Fabric",
    "api.example.com",
    "url-secret",
    "private-",
    "Users",
    "Flint",
    "server",
    "home/flint",
    WINDOWS_PATH,
    FORWARD_WINDOWS_PATH,
    UNIX_PATH,
    UNC_PATH,
    RELATIVE_PATH
  ];
  for (const value of forbidden) {
    assert.equal(sanitized.includes(value), false, `${value} must be redacted`);
  }
  assert.match(sanitized, /\[redacted\]/);
  assert.match(sanitized, /Bearer \[redacted\]/);
  assert.match(sanitized, /\[provider\]/);
  assert.match(sanitized, /\[url\]/);
  assert.match(sanitized, /\[path\]/);
});
