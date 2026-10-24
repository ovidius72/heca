import { appendFileSync, constants } from "node:fs";
import { Plugin } from "@opencode/plugin/tui";

type StatusReport = {
  driver: string;
  name: string;
  status: string;
  task: string | null;
  detail: string | null;
  turn_id: string | null;
  model?: string | null;
  session_id?: string | null;
  session_title?: string | null;
};

export default Plugin.define({
  id: "heca.terminal-status",
  setup(context) {
    if (process.env.HECA_AGENT_DRIVER !== "opencode") return;
    const endpoint = process.env.HECA_AGENT_REPORT;
    const reports = JSON.parse(process.env.HECA_AGENT_EVENTS ?? "{}") as Record<string, StatusReport>;
    if (!endpoint) return;
    const stop = context.data.listen(({ details }) => {
        const report = reports[details.type];
        if (!report) return;
        const route = context.ui.router.current();
        if (route.type !== "session") return;
        // Each terminal follows its own active session. Other clients may share the same server.
        if (!("sessionID" in details.data) || details.data.sessionID !== route.sessionID) return;
        const session = context.data.session.get(route.sessionID);
        const task = session?.title ?? null;
        try {
          appendFileSync(endpoint, JSON.stringify({
            ...report, task,
            model: session?.model?.id ?? null,
            session_id: route.sessionID,
            session_title: session?.title ?? null,
          }) + "\n", { flag: constants.O_WRONLY | constants.O_APPEND });
        } catch {
          // A closed terminal must not interrupt OpenCode's event handlers.
        }
    });
    return stop;
  },
});
