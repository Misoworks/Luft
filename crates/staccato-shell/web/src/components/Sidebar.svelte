<script lang="ts">
  import AppIcon from "./AppIcon.svelte";
  import Icon from "./Icon.svelte";
  import { workspaceWheelOffset } from "../lib/workspace_wheel";
  import { sendAction } from "../shell/bridge";
  import type { DockApp, ShellSnapshot, WorkspaceItem } from "../shell/model";

  let { snapshot }: { snapshot: ShellSnapshot } = $props();

  function workspaceScroll(event: WheelEvent) {
    const offset = workspaceWheelOffset(event);
    if (offset === 0) return;
    sendAction({ type: "workspace-relative", offset });
  }

  function switchWorkspace(workspace: WorkspaceItem) {
    sendAction({ type: "workspace-switch", workspace: workspace.id });
  }

  function launch(app: DockApp) {
    sendAction({ type: "dock-launch", command: app.command });
  }
</script>

<nav class="shell-sidebar">
  <div class="sidebar-actions">
    <button type="button" class="sidebar-action" aria-label="Open launcher" onclick={() => sendAction({ type: "open-launcher" })}>
      <Icon name="search" />
    </button>
    <button type="button" class="sidebar-action" aria-label="Open overview" onclick={() => sendAction({ type: "toggle-overview" })}>
      <Icon name="app" />
    </button>
    <button
      type="button"
      class="sidebar-action"
      aria-label="Quick settings"
      onclick={() => sendAction({ type: "toggle-quick-settings" })}
    >
      <Icon name="settings" />
    </button>
  </div>

  <div class="sidebar-workspaces" onwheel={workspaceScroll}>
    {#each snapshot.workspaces as workspace (workspace.id)}
      <button
        type="button"
        class="sidebar-workspace"
        class:is-active={workspace.active}
        aria-label={workspace.name}
        onclick={() => switchWorkspace(workspace)}
      >
        <strong>{workspace.name}</strong>
        <span>{workspace.mode}</span>
      </button>
    {/each}
  </div>

  <div class="sidebar-pins">
    {#each snapshot.dockApps.slice(0, 5) as app (app.command)}
      <button
        type="button"
        class="sidebar-pin"
        class:is-active={app.active}
        class:is-running={app.running}
        aria-label={app.label}
        onclick={() => launch(app)}
      >
        <AppIcon {app} />
        <span class="running-dot"></span>
      </button>
    {/each}
  </div>
</nav>
