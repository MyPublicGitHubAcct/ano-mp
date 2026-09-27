<script lang="ts">
  // The dialog `ui.dialog` asks for, if any.
  import { ui } from "$lib/state/ui.svelte";
  import ChooseCoverDialog from "./ChooseCoverDialog.svelte";
  import FindArtistDialog from "./FindArtistDialog.svelte";
  import FindDetailsDialog from "./FindDetailsDialog.svelte";
  import PrefsDialog from "./PrefsDialog.svelte";

  const close = () => (ui.dialog = null);
</script>

{#if ui.dialog?.kind === "findDetails"}
  {#key ui.dialog}<FindDetailsDialog album={ui.dialog.album} onclose={close} />{/key}
{:else if ui.dialog?.kind === "chooseCover"}
  {#key ui.dialog}<ChooseCoverDialog album={ui.dialog.album} onclose={close} />{/key}
{:else if ui.dialog?.kind === "findArtist"}
  {#key ui.dialog}<FindArtistDialog artist={ui.dialog.artist} info={ui.dialog.info} onclose={close} />{/key}
{:else if ui.dialog?.kind === "prefs"}
  {#key ui.dialog}<PrefsDialog track={ui.dialog.track} album={ui.dialog.album} onclose={close} />{/key}
{/if}
