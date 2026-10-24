import { appendFileSync, constants } from "node:fs";

// No terminal writes: Pi owns its terminal and the private report file carries our events.
export default function (api) {
  if (process.env.HECA_AGENT_DRIVER !== "pi") return;
  const endpoint = process.env.HECA_AGENT_REPORT;
  const reports = JSON.parse(process.env.HECA_AGENT_EVENTS ?? "{}");
  if (!endpoint) return;
  const events = new Set(Object.keys(reports).map((key) => key.split(":")[0]));
  let outcome;
  if (events.has("agent_settled")) {
    // Settlement happens after retries/continuations. The preceding native boundary supplies
    // its final outcome; a lifecycle exit by itself must never be interpreted as success.
    api.on("agent_before_settle", (native) => { outcome = native.outcome; });
    api.on("agent_start", () => { outcome = undefined; });
  }
  for (const event of events) {
    api.on(event, (native, context) => {
      const key = event === "agent_settled"
        ? `${event}:${native.aborted ? "aborted" : outcome}` : event;
      const report = reports[key] ?? reports[event];
      if (!report) return;
      try {
        appendFileSync(endpoint, JSON.stringify({
          ...report,
          model: context.model?.name ?? context.model?.id ?? null,
          session_id: context.sessionManager.getSessionId(),
          session_title: api.getSessionName() ?? null,
        }) + "\n", { flag: constants.O_WRONLY | constants.O_APPEND });
      } catch {
        // Closing a Heca terminal removes its report file. It must not break Pi's lifecycle.
      }
    });
  }
}
