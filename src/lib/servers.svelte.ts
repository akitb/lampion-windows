// The servers part in the UI: keeps `app.servers` in sync with `servers://status`, holds the setup
// banner's state (`servers://setup`) and wraps the servers commands.
import { call, on } from "./api";
import { app } from "./state.svelte";
import type { PhpInfo, ServerId, ServerStatus, SetupState } from "./types";

type Statuses = Record<ServerId, ServerStatus>;

let watching = false;

/** Keeps `app.servers` up to date; the core sends a status only when something really changed. */
export function startServerStatus(): void {
  if (watching) return;
  watching = true;
  let heard = false;
  on<Statuses>("servers://status", (s) => {
    heard = true;
    app.servers = s;
  });
  call<Statuses>("server_status")
    .then((s) => {
      if (!heard) app.servers = s;
    })
    .catch(() => {});
}

/** The setup banner's state; `null` until the core answered. */
export const setup = $state({ state: null as SetupState | null });

let watchingSetup = false;

export function startSetupState(): void {
  if (watchingSetup) return;
  watchingSetup = true;
  let heard = false;
  on<SetupState>("servers://setup", (s) => {
    heard = true;
    setup.state = s;
  });
  call<SetupState>("setup_state")
    .then((s) => {
      if (!heard) setup.state = s;
    })
    .catch(() => {});
}

/** The last failed action per server, shown on its card until the next action. */
export const actionErrors = $state({ apache: null, mariadb: null } as Record<ServerId, string | null>);

/** Runs a server action; a failure is kept for the card (the activity log has the details). */
async function act(id: ServerId, run: () => Promise<void>): Promise<void> {
  actionErrors[id] = null;
  try {
    await run();
  } catch (e) {
    actionErrors[id] = String(e);
  }
}

const ids: ServerId[] = ["apache", "mariadb"];

/** Every server command (`config_edit_file` belongs to the config part; the Overview's menu uses it). */
export const servers = {
  start: (server: ServerId) => act(server, () => call("server_start", { server })),
  stop: (server: ServerId) => act(server, () => call("server_stop", { server })),
  restart: (server: ServerId) => act(server, () => call("server_restart", { server })),
  /** Each server on its own, so a failure shows on its card (the core queues them one after the other). */
  startAll: () => Promise.all(ids.filter((id) => app.servers[id].state === "stopped").map(servers.start)),
  stopAll: () => Promise.all(ids.filter((id) => app.servers[id].state === "running").map(servers.stop)),
  phpInfo: () => call<PhpInfo>("php_info"),
  setupRun: () => call("setup_run"),
  setupRetry: () => call("setup_retry"),
  setupCancel: () => call("setup_cancel"),
  /** Validates another Lampion folder (before anything is installed); rejects with the reason. */
  setupChooseRoot: (path: string) => call<SetupState>("setup_choose_root", { path }),
  setupInstallRuntime: () => call("setup_install_runtime"),
  /** General › Locations › Move…: resolves with the new folder (follow `servers://moving`). */
  moveRoot: (target: string) => call<string>("servers_move_root", { target }),
  moving: () => call<boolean>("servers_moving"),
  /** General › Run the servers as Windows services (one UAC prompt). */
  useServices: (enabled: boolean) => call("servers_use_services", { enabled }),
  editConfig: (id: "apache" | "php" | "mariadb" | "phpmyadmin") => call("config_edit_file", { id }),
};
