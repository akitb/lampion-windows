// State of the settings pages. It lives here, outside the page components, so a draft survives
// switching pages (the sidebar shows the pencil while a page has unsaved edits).
import "./config.css";
import { tick, untrack } from "svelte";
import { call, on } from "$lib/api";
import { app, setUnsaved } from "$lib/state.svelte";
import type {
  ApacheForm, ApacheSettings, ConfigCheck, ConfigFileId, Locations, MariadbForm, MariadbSettings, Pane, PhpForm, PhpModules,
  PhpSettings, SaveOutcome, ServerId,
} from "$lib/types";

export type Message = { text: string; ok: boolean } | null;

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);
const copy = <T>(v: T): T => structuredClone($state.snapshot(v) as T);

/** One settings page: what the file says (`saved`), the edits (`draft`) and the save flow. */
export class SettingsForm<T, I extends { settings: T; file: string; hasBackup: boolean }> {
  saved = $state<T | null>(null);
  draft = $state<T | null>(null);
  info = $state<I | null>(null);
  check = $state<ConfigCheck>({ problems: [], warnings: [] });
  loadError = $state<string | null>(null);
  saving = $state(false);
  message = $state<Message>(null);
  /** The server's rejection or a write error, shown as an alert. */
  error = $state<string | null>(null);
  dirty = $derived(this.saved !== null && this.draft !== null && !same(this.draft, this.saved));
  #checks = 0;
  #flash: ReturnType<typeof setTimeout> | undefined;

  constructor(
    readonly pane: Pane,
    readonly id: "apache" | "php" | "mariadb",
    /** The server a save restarts (PHP runs inside Apache). */
    readonly server: ServerId,
    readonly serverName: string,
  ) {
    $effect.root(() => {
      $effect(() => {
        const dirty = this.dirty;
        untrack(() => setUnsaved(this.pane, dirty));
      });
      $effect(() => {
        const d = $state.snapshot(this.draft);
        if (d) this.validate(d as T);
      });
    });
  }

  get file(): ConfigFileId {
    return this.id;
  }

  get running() {
    return app.servers[this.server].state === "running";
  }

  /** The first problem of a field, for its red outline. */
  problem(field: string): string | undefined {
    return this.check.problems.find((p) => p.field === field)?.message;
  }

  warning(field: string): string | undefined {
    return this.check.warnings.find((p) => p.field === field)?.message;
  }

  /** Reads the file. Keeps unsaved edits unless `force`. */
  async load(force = false) {
    if (this.dirty && !force) return;
    try {
      const form = await call<I>(`config_${this.id}_load`);
      this.info = form;
      this.saved = form.settings;
      this.draft = copy(form.settings);
      this.loadError = null;
    } catch (e) {
      this.loadError = String(e);
    }
  }

  private async validate(d: T) {
    const n = ++this.#checks;
    try {
      const c = await call<ConfigCheck>(`config_${this.id}_check`, { draft: d });
      if (n === this.#checks) this.check = c;
    } catch {
      // The save checks again; a failed live check only hides the hints.
    }
  }

  revert() {
    if (this.saved) this.draft = copy(this.saved);
  }

  flash(text: string, ok: boolean) {
    clearTimeout(this.#flash);
    this.message = { text, ok };
    this.#flash = setTimeout(() => (this.message = null), ok ? 5000 : 10000);
  }

  /** Saves (the server checks the change first), re-reads the file and restarts the server if asked. */
  async save(restart: boolean) {
    if (!this.saved || !this.draft || this.saving) return;
    this.saving = true;
    this.message = null;
    try {
      const r = await call<SaveOutcome>(`config_${this.id}_save`, { saved: copy(this.saved), draft: copy(this.draft) });
      if (r.problems.length) {
        this.check = { problems: r.problems, warnings: this.check.warnings };
        return;
      }
      await this.load(true);
      if (restart) {
        this.flash(`Saved. Restarting ${this.serverName}…`, true);
        call("server_restart", { server: this.server }).catch((e) => this.flash(`Saved, but ${this.serverName} did not restart: ${e}`, false));
      } else {
        this.flash(`Saved. Restart ${this.serverName} to apply the changes.`, true);
      }
    } catch (e) {
      this.error = String(e);
    } finally {
      this.saving = false;
    }
  }

  /** Puts back the file as it was before Lampion first changed it. */
  async restore() {
    try {
      await call("config_restore", { id: this.file });
      await this.load(true);
      this.flash(`Restored the original file. Restart ${this.serverName} to apply it.`, true);
    } catch (e) {
      this.error = String(e);
    }
  }
}

/** Live ports and paths; refreshed by the core every 10 s when a config file was edited by hand. */
export const config = $state({ locations: null as Locations | null });

export async function loadLocations() {
  try {
    config.locations = await call<Locations>("config_locations");
  } catch {
    // Shown as missing paths; the next event fills them in.
  }
}

/** Switches to the Logs page showing one log. The logs part listens for `logs:show` (detail: source id:
 *  activity | apacheError | apacheAccess | php | mariadb | mariadbSlow | mariadbGeneral). */
export async function showLog(source: string) {
  app.pane = "logs";
  await tick();
  window.dispatchEvent(new CustomEvent("logs:show", { detail: { source } }));
}

/** 0 when the user asked for reduced motion. */
export function motion(ms: number) {
  return matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : ms;
}

export const apacheForm = new SettingsForm<ApacheSettings, ApacheForm>("apache", "apache", "apache", "Apache");
/** PHP runs inside Apache, so its changes apply when Apache restarts. */
export const phpForm = new SettingsForm<PhpSettings, PhpForm>("php", "php", "apache", "Apache");
export const mariadbForm = new SettingsForm<MariadbSettings, MariadbForm>("mariadb", "mariadb", "mariadb", "MariaDB");

/** PHP's version and loaded extensions (`php -m`), read once and again after each PHP save. */
export const php = $state({ info: null as PhpModules | null, error: null as string | null });

export async function loadPhpModules() {
  try {
    php.info = await call<PhpModules>("config_php_modules");
    php.error = null;
  } catch (e) {
    php.error = String(e);
  }
}

void on<Locations>("config://locations", (l) => {
  config.locations = l;
  // A hand-edited file shows up on pages without unsaved edits.
  for (const f of [apacheForm, phpForm, mariadbForm]) if (f.saved && !f.dirty) void f.load();
});
