<script lang="ts">
  // The app around the library (PLAN.md F6, F7, F21): controls in the menu
  // bar, the mini player, track-change notifications, and the keyboard
  // shortcuts.
  import { shell } from "$lib/api";
  import { t } from "$lib/i18n";
  import { appSettings } from "$lib/state/settings.svelte";
  import { attempt } from "$lib/state/toasts.svelte";
  import { ui } from "$lib/state/ui.svelte";

  const window = $derived(appSettings.current.window);
</script>

<h3>{t("general.menuBar")}</h3>
<label class="switch">
  <input
    id="setting-menuBarControls"
    type="checkbox"
    checked={window.menuBarControls}
    onchange={(event) => {
      const on = event.currentTarget.checked;
      appSettings.save((next) => (next.window.menuBarControls = on));
    }}
  />
  <span>
    <span class="title">{t("general.menuBarControls")}</span>
    <span class="hint">{t("general.menuBarControlsHint")}</span>
  </span>
</label>

<h3>{t("general.miniPlayer")}</h3>
<label class="switch">
  <input
    id="setting-miniPlayerOnTop"
    type="checkbox"
    checked={window.miniPlayerOnTop}
    onchange={(event) => {
      const on = event.currentTarget.checked;
      appSettings.save((next) => (next.window.miniPlayerOnTop = on));
    }}
  />
  <span>
    <span class="title">{t("general.onTop")}</span>
    <span class="hint">{t("general.onTopHint")}</span>
  </span>
</label>
<div class="actions">
  <button onclick={() => attempt(shell.toggleMiniPlayer)}>{t("general.openMini")}</button>
</div>

<h3>{t("general.notifications")}</h3>
<label class="switch">
  <input
    id="setting-trackNotifications"
    type="checkbox"
    checked={window.trackNotifications}
    onchange={(event) => {
      const on = event.currentTarget.checked;
      appSettings.save((next) => (next.window.trackNotifications = on));
    }}
  />
  <span>
    <span class="title">{t("general.trackNotifications")}</span>
    <span class="hint">{t("general.trackNotificationsHint")}</span>
  </span>
</label>

<h3>{t("general.keyboard")}</h3>
<p class="hint">{t("general.keyboardHint")}</p>
<div class="actions">
  <button onclick={() => (ui.dialog = { kind: "shortcuts" })}>{t("general.showShortcuts")}</button>
</div>
