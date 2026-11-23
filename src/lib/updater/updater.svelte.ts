// The update banner's state (`update://state`) and its actions. Owned by the app part.
import { call, core, on } from "$lib/api";
import type { UpdateState } from "$lib/types";

class Updater {
  state = $state.raw<UpdateState>({ state: "idle" });
  /** The running version, for "Version X is installed". */
  version = $state("");
}

export const updater = new Updater();

let started = false;

/** Follows the updater (once). The first answer is used only if no event came before it. */
export function startUpdater(): void {
  if (started) return;
  started = true;
  let heard = false;
  void on<UpdateState>("update://state", (s) => {
    heard = true;
    updater.state = s;
  });
  call<UpdateState>("update_state").then((s) => {
    if (!heard) updater.state = s;
  }, () => {});
  core.appInfo().then((i) => (updater.version = i.version), () => {});
}

export const update = {
  /** Check Now / Try Again: the answer always shows. */
  check: () => call("update_check", { userInitiated: true }),
  install: () => call("update_install"),
  cancel: () => call("update_cancel"),
  skip: () => call("update_skip"),
  later: () => call("update_later"),
};
