// Run: node --experimental-vm-modules heca/tests/fixtures/agent_reporting.mjs
// Run the shipped adapters unchanged, with fixtures for the native application APIs.
import assert from "node:assert/strict";
import * as fs from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { stripTypeScriptTypes } from "node:module";
import { SourceTextModule, SyntheticModule } from "node:vm";

const root = fs.mkdtempSync(join(tmpdir(), "heca-reporting-"));
const original = { ...process.env };
const assets = new URL("../../src/agent/drivers/assets/", import.meta.url);
const reports = (path) => fs.readFileSync(path, "utf8").trim().split("\n").filter(Boolean).map(JSON.parse);

async function adapter(name) {
  let code = fs.readFileSync(new URL(name, assets), "utf8");
  if (name.endsWith(".ts")) code = stripTypeScriptTypes(code);
  const module = new SourceTextModule(code);
  await module.link(async (specifier) => {
    const exports = specifier === "node:fs" ? fs
      : specifier === "@opencode/plugin/tui" ? { Plugin: { define: (plugin) => plugin } }
      : assert.fail(`unexpected import: ${specifier}`);
    return new SyntheticModule(Object.keys(exports), function () {
      for (const [key, value] of Object.entries(exports)) this.setExport(key, value);
    });
  });
  await module.evaluate();
  return module.namespace.default;
}

function environment(driver, events) {
  const endpoint = join(root, `${driver}.jsonl`);
  fs.writeFileSync(endpoint, "");
  process.env.HECA_AGENT_DRIVER = driver;
  process.env.HECA_AGENT_REPORT = endpoint;
  process.env.HECA_AGENT_EVENTS = JSON.stringify(Object.fromEntries(events.map((event) => [event, {
    driver, name: driver, status: "working", task: null, detail: null, turn_id: null,
  }])));
  return endpoint;
}

try {
  const pi = await adapter("pi.mjs");
  const piPath = environment("pi", ["session_start", "agent_end"]);
  const handlers = new Map();
  let title = "First session";
  pi({ on: (event, handler) => handlers.set(event, handler), getSessionName: () => title });
  assert.deepEqual([...handlers.keys()], ["session_start", "agent_end"]);
  handlers.get("session_start")({}, { model: { id: "model-a", name: "Model A" }, sessionManager: { getSessionId: () => "pi-a" } });
  title = undefined;
  handlers.get("agent_end")({}, { model: undefined, sessionManager: { getSessionId: () => "pi-b" } });
  const [first, second] = reports(piPath);
  assert.equal(first.model, "Model A");
  assert.equal(first.session_id, "pi-a");
  assert.equal(first.session_title, "First session");
  assert.equal(second.session_id, "pi-b");
  assert.equal(second.model, null);
  assert.equal(second.session_title, null);
  fs.unlinkSync(piPath);
  handlers.get("agent_end")({}, { sessionManager: { getSessionId: () => "pi-b" } });
  assert.equal(fs.existsSync(piPath), false, "Pi must not recreate a closed pane's endpoint");
  process.env.HECA_AGENT_DRIVER = "other";
  pi({ on: () => assert.fail("foreign driver registered callbacks") });

  const settledPath = environment("pi", ["agent_start", "agent_settled:completed", "agent_settled:error", "agent_settled:aborted"]);
  const choices = JSON.parse(process.env.HECA_AGENT_EVENTS);
  choices["agent_settled:completed"].status = "finished";
  choices["agent_settled:error"].status = "error";
  choices["agent_settled:aborted"].status = "waiting_input";
  process.env.HECA_AGENT_EVENTS = JSON.stringify(choices);
  const settled = new Map();
  const context = { sessionManager: { getSessionId: () => "settlement" } };
  pi({
    on: (event, handler) => settled.set(event, [...(settled.get(event) ?? []), handler]),
    getSessionName: () => "Settlement test",
  });
  const emitPi = (event, native) => { for (const handler of settled.get(event) ?? []) handler(native, context); };
  for (const outcome of ["completed", "error", "aborted"]) {
    emitPi("agent_start", {});
    emitPi("agent_before_settle", { outcome });
    emitPi("agent_settled", { aborted: outcome === "aborted" });
  }
  assert.deepEqual(reports(settledPath).map((report) => report.status),
    ["working", "finished", "working", "error", "working", "waiting_input"]);
  emitPi("agent_start", {});
  emitPi("agent_settled", { aborted: false });
  assert.equal(reports(settledPath).at(-1).status, "working", "unknown outcome must not guess completion");

  const opencode = await adapter("opencode.ts");
  const opencodePath = environment("opencode", ["session.created", "session.idle"]);
  let listener;
  let stopped = false;
  let route = { type: "session", sessionID: "oc-a" };
  const sessions = new Map([
    ["oc-a", { id: "oc-a", title: "OpenCode A", model: { id: "model-a", providerID: "provider-a" } }],
    ["oc-b", { id: "oc-b", model: { id: "model-b", providerID: "provider-b" } }],
  ]);
  const stop = opencode.setup({
    data: {
      listen: (handler) => { listener = handler; return () => { stopped = true; }; },
      session: { get: (id) => sessions.get(id) },
    },
    ui: { router: { current: () => route } },
  });
  const emit = (type, sessionID) => listener({ details: { type, data: { sessionID } } });
  emit("session.created", "foreign-session");
  emit("unselected.event", "oc-a");
  route = { type: "home" };
  emit("session.created", "oc-a");
  assert.equal(fs.readFileSync(opencodePath, "utf8"), "");
  route = { type: "session", sessionID: "oc-a" };
  emit("session.created", "oc-a");
  route = { type: "session", sessionID: "oc-b" };
  emit("session.idle", "oc-a");
  emit("session.idle", "oc-b");
  const [a, b] = reports(opencodePath);
  assert.equal(a.model, "model-a");
  assert.equal(a.session_id, "oc-a");
  assert.equal(a.session_title, "OpenCode A");
  assert.equal(b.model, "model-b");
  assert.equal(b.session_id, "oc-b");
  assert.equal(b.session_title, null);
  fs.unlinkSync(opencodePath);
  emit("session.idle", "oc-b");
  assert.equal(fs.existsSync(opencodePath), false, "OpenCode must not recreate a closed pane's endpoint");
  stop();
  assert.equal(stopped, true);
  process.env.HECA_AGENT_DRIVER = "other";
  opencode.setup({ data: { listen: () => assert.fail("foreign driver registered callbacks") } });
  console.log("Pi and OpenCode metadata, session isolation, selection and closed-endpoint checks passed.");
} finally {
  for (const key of Object.keys(process.env)) if (!(key in original)) delete process.env[key];
  Object.assign(process.env, original);
  fs.rmSync(root, { recursive: true, force: true });
}
