<script lang="ts">
  // Shown instead of the content while MariaDB can't be reached. When root has a password (the
  // "root without password" setting is off), it asks for it; the password is kept in memory only.
  import { KeyRound, Plug } from "@lucide/svelte";
  import IconTile from "$lib/components/IconTile.svelte";
  import { dbapi } from "./dbapi.svelte";
  import { connectWithPassword, reloadAll, ui } from "./model.svelte";

  let password = $state("");
  let trying = $state(false);
  const denied = $derived((ui.connectionError ?? "").startsWith("ERROR 1045"));

  async function attempt(e?: SubmitEvent) {
    e?.preventDefault();
    trying = true;
    if (denied) await connectWithPassword(password);
    else await reloadAll();
    trying = false;
  }
</script>

<form class="wrap" onsubmit={attempt}>
  <span class="tile"><IconTile icon={denied ? KeyRound : Plug} tint="var(--red)" size={56} glyph={0.5} /></span>
  <div class="titles">
    <h2>{denied ? "MariaDB Asks for the Root Password" : "Can't Connect to the Database"}</h2>
    <p>
      {#if denied}
        root has a password on this server. Enter it to connect; Lampion keeps it only until it quits.
      {:else}
        MariaDB must be running. Start it from the Lampion main window, then try again.
      {/if}
    </p>
  </div>
  <pre class="message">{ui.connectionError}</pre>
  {#if denied}
    <input type="password" bind:value={password} placeholder="root password" aria-label="Root password" autocomplete="current-password" />
  {/if}
  <div class="buttons">
    <button type="button" class="pushbtn large" onclick={() => dbapi.showMainWindow()} title="Show the main window, where MariaDB can be started">Open Lampion</button>
    <button type="submit" class="pushbtn large prominent" disabled={trying}>{denied ? "Connect" : "Try Again"}</button>
  </div>
</form>

<style>
  .wrap {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 14px;
    height: 100%;
    padding: 32px;
    text-align: center;
  }
  .tile { display: inline-flex; color: #fff; }
  .titles { display: flex; flex-direction: column; gap: 6px; }
  h2 { margin: 0; font-size: 17px; font-weight: 600; }
  p { margin: 0; max-width: 460px; font-size: 12px; color: var(--text-2); }
  .message {
    width: 100%;
    max-width: 520px;
    margin: 0;
    padding: 12px;
    border: 1px solid color-mix(in srgb, var(--text) 8%, transparent);
    border-radius: 10px;
    background: color-mix(in srgb, var(--text) 4%, transparent);
    color: var(--text-2);
    font-family: var(--mono);
    font-size: 11px;
    text-align: left;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    user-select: text;
  }
  input { width: 260px; height: 28px; font-size: 13px; }
  .buttons { display: flex; gap: 10px; margin-top: 4px; }
</style>
