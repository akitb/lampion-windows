<script lang="ts">
  // A settings page: the centered form column scrolls; the save bar stays at the bottom.
  // A save the server rejected shows as an alert ("The settings could not be saved").
  import type { Snippet } from "svelte";
  import SaveBar from "./SaveBar.svelte";
  import Dialog from "./Dialog.svelte";
  import type { SettingsForm } from "./forms.svelte";
  let {
    form,
    onsave,
    children,
  }: { form?: SettingsForm<any, any>; onsave?: (restart: boolean) => void; children: Snippet } = $props();
</script>

<div class="page">
  <div class="scroll">
    <div class="column">{@render children()}</div>
  </div>
  {#if form}
    <SaveBar
      dirty={form.dirty}
      problem={form.check.problems[0]?.message}
      saving={form.saving}
      restartName={form.running ? form.serverName : null}
      message={form.message}
      onrevert={() => form.revert()}
      onsave={(restart) => (onsave ? onsave(restart) : form.save(restart))}
    />
    <Dialog
      bind:open={() => form.error !== null, (v) => { if (!v) form.error = null; }}
      title="The settings could not be saved"
      message={form.error ?? ""}
    />
  {/if}
</div>

<style>
  .page { display: flex; flex-direction: column; height: 100%; }
  .scroll { flex: 1; min-height: 0; overflow: auto; }
  .column { width: min(704px, 100% - 40px); margin: 0 auto; padding: 20px 0; }
</style>
