<script lang="ts">
  import { sendAction } from "../shell/bridge";
  import type { PanelApp, ShellSnapshot, WindowItem } from "../shell/model";

  let { snapshot }: { snapshot: ShellSnapshot } = $props();
  const app = $derived(snapshot.panelApps.find((entry) => entry.command === snapshot.panelMenuCommand));
  const window = $derived(app ? matchedWindow(app, snapshot.windows) : undefined);
  const isRunning = $derived(Boolean(window ?? app?.running));
  const canLaunch = $derived(Boolean(app && launchable(app)));

  function close() {
    sendAction({ type: "panel-menu-close" });
  }

  function open(app: PanelApp, forceNew = false) {
    close();
    if (!forceNew && window) {
      sendAction({ type: "window-activate", window: window.id });
    } else if (app.windowId !== undefined && !forceNew) {
      sendAction({ type: "window-activate", window: app.windowId });
    } else {
      sendAction({ type: "panel-launch", command: app.command });
    }
  }

  function unpin(app: PanelApp) {
    close();
    sendAction({ type: "panel-unpin", command: app.command });
  }

  function pin(app: PanelApp) {
    close();
    sendAction({ type: "panel-pin", label: app.label, command: app.command });
  }

  function minimize(window: WindowItem) {
    close();
    sendAction({ type: "window-minimize", window: window.id });
  }

  function closeWindow(window: WindowItem) {
    close();
    sendAction({ type: "window-close", window: window.id });
  }

  function forceQuit(app: PanelApp) {
    close();
    sendAction({ type: "panel-force-quit", command: app.command });
  }

  function matchedWindow(app: PanelApp, windows: WindowItem[]) {
    return (
      windows.find((window) => window.active && app.windowIds.includes(window.id)) ??
      windows.find((window) => window.visible && app.windowIds.includes(window.id)) ??
      windows.find((window) => app.windowIds.includes(window.id)) ??
      windows.find((window) => window.active && window.visible && windowMatchesApp(window, app)) ??
      windows.find((window) => window.visible && windowMatchesApp(window, app)) ??
      windows.find((window) => windowMatchesApp(window, app))
    );
  }

  function windowMatchesApp(window: WindowItem, app: PanelApp) {
    if (app.windowIds.includes(window.id)) return true;
    if (app.windowId === window.id) return true;
    const command = commandName(app.command);
    const label = app.label.toLowerCase();
    return [window.appId, window.title].some((value) => {
      const text = value?.toLowerCase() ?? "";
      return Boolean(text && ((command && text.includes(command)) || (label && text.includes(label))));
    });
  }

  function commandName(command: string) {
    return command.trim().split(/\s+/)[0]?.split("/").at(-1)?.replace(/^['"]|['"]$/g, "").toLowerCase() ?? "";
  }

  function launchable(app: PanelApp) {
    return !app.command.startsWith("window:") && !app.command.startsWith("window-group:");
  }
</script>

<section class="panel-menu-shell">
  {#if app}
    <div class="panel-menu" role="menu" tabindex="-1" data-command={app.command} onpointerdown={(event) => event.stopPropagation()}>
      <strong>{app.label}</strong>
      {#if window}
        {#if !window.active}
          <button type="button" class="panel-menu-item" role="menuitem" onclick={() => open(app)}>
            <span>Focus</span>
          </button>
        {/if}
        {#if app.pinned || canLaunch}
          <button type="button" class="panel-menu-item" role="menuitem" onclick={() => open(app, true)}>
            <span>Open New Window</span>
          </button>
        {/if}
        <button type="button" class="panel-menu-item" role="menuitem" onclick={() => minimize(window)}>
          <span>Minimize</span>
        </button>
        <button type="button" class="panel-menu-item" role="menuitem" onclick={() => closeWindow(window)}>
          <span>Close Window</span>
        </button>
        <button type="button" class="panel-menu-item is-danger" role="menuitem" onclick={() => forceQuit(app)}>
          <span>Force Quit</span>
        </button>
      {:else if isRunning}
        {#if canLaunch}
          <button type="button" class="panel-menu-item" role="menuitem" onclick={() => open(app, true)}>
            <span>Open New Window</span>
          </button>
        {/if}
        <button type="button" class="panel-menu-item is-danger" role="menuitem" onclick={() => forceQuit(app)}>
          <span>Force Quit</span>
        </button>
      {:else}
        <button type="button" class="panel-menu-item" role="menuitem" onclick={() => open(app, true)}>
          <span>Open</span>
        </button>
      {/if}
      {#if app.pinned}
        <button type="button" class="panel-menu-item" role="menuitem" onclick={() => unpin(app)}>
          <span>Unpin from Panel</span>
        </button>
      {:else if canLaunch}
        <button type="button" class="panel-menu-item" role="menuitem" onclick={() => pin(app)}>
          <span>Pin to Panel</span>
        </button>
      {/if}
    </div>
  {/if}
</section>
