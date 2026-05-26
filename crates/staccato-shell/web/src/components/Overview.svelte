<script lang="ts">
  import DebugMeter from "./DebugMeter.svelte";
  import Icon from "./Icon.svelte";
  import { sendAction } from "../shell/bridge";
  import { filteredApplications, selectedApplication } from "../lib/overview_state";
  import { geometryStyle } from "../lib/geometry";
  import type { ApplicationItem, ShellSnapshot, WindowItem, WorkspaceItem } from "../shell/model";

  let {
    snapshot,
    query,
    selection,
    setQuery,
    setSelection,
  }: {
    snapshot: ShellSnapshot;
    query: string;
    selection: number;
    setQuery: (query: string) => void;
    setSelection: (selection: number) => void;
  } = $props();

  const active = $derived(snapshot.workspaces.find((workspace) => workspace.active));
  const visibleApps = $derived(filteredApplications(snapshot, query).slice(0, 32));
  const clampedSelection = $derived.by(() => {
    if (selection < 0 || visibleApps.length <= 0) return -1;
    return Math.max(0, Math.min(selection, visibleApps.length - 1));
  });

  function focusSearch(node: HTMLInputElement) {
    requestAnimationFrame(() => node.focus());
  }

  function workspaceScroll(event: WheelEvent) {
    sendAction({ type: "workspace-relative", offset: event.deltaY > 0 ? 1 : -1 });
  }

  function searchChanged(event: Event) {
    const value = (event.currentTarget as HTMLInputElement).value;
    setQuery(value);
    setSelection(value.trim() ? 0 : -1);
  }

  function searchKeydown(event: KeyboardEvent) {
    if (["ArrowDown", "ArrowRight"].includes(event.key)) {
      event.preventDefault();
      moveSelection(1);
      return;
    }
    if (["ArrowUp", "ArrowLeft"].includes(event.key)) {
      event.preventDefault();
      moveSelection(-1);
      return;
    }
    if (event.key !== "Enter") return;
    const app = selectedApplication(visibleApps, clampedSelection);
    if (!app) {
      sendAction({ type: "open-launcher" });
      return;
    }
    sendAction({ type: "app-launch", command: app.command });
  }

  function moveSelection(offset: number) {
    if (visibleApps.length <= 0) return;
    const base = clampedSelection < 0 ? (offset > 0 ? -1 : 0) : clampedSelection;
    setSelection((base + offset + visibleApps.length) % visibleApps.length);
  }

  function workspaceWindows(workspace: string) {
    return snapshot.windows.filter((window) => window.workspace === workspace && window.visible);
  }

  function dragHasWindow(event: DragEvent) {
    return Array.from(event.dataTransfer?.types ?? []).includes("application/x-staccato-window");
  }

  function draggedWindow(event: DragEvent) {
    const value = event.dataTransfer?.getData("application/x-staccato-window");
    const window = value ? Number(value) : 0;
    return Number.isFinite(window) && window > 0 ? window : undefined;
  }

  function windowDragStart(event: DragEvent, window: WindowItem) {
    event.stopPropagation();
    event.dataTransfer?.setData("application/x-staccato-window", String(window.id));
    event.dataTransfer?.setData("text/plain", window.title);
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = "move";
    }
  }

  function dropWindow(event: DragEvent, workspace: WorkspaceItem) {
    const window = draggedWindow(event);
    if (!window) return;
    event.preventDefault();
    sendAction({ type: "window-move", window, workspace: workspace.id });
  }

  function workspaceKeydown(event: KeyboardEvent, workspace: WorkspaceItem) {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    sendAction({ type: "workspace-switch", workspace: workspace.id });
  }

  function launchApp(app: ApplicationItem) {
    sendAction({ type: "app-launch", command: app.command });
  }

  function pinApp(event: MouseEvent, app: ApplicationItem) {
    event.preventDefault();
    if (app.pinned) {
      sendAction({ type: "dock-unpin", command: app.command });
    } else {
      sendAction({ type: "dock-pin", label: app.name, command: app.command, icon: app.icon });
    }
  }
</script>

<section class="shell-overview">
  <header class="overview-top">
    <div class="overview-title">
      <strong>{active?.name ?? "Workspace"}</strong>
      <span>{snapshot.time}</span>
    </div>

    <label class="overview-search">
      <Icon name="search" />
      <input
        use:focusSearch
        class="overview-search-input"
        type="search"
        autocomplete="off"
        spellcheck={false}
        placeholder="Search"
        value={query}
        oninput={searchChanged}
        onkeydown={searchKeydown}
      />
    </label>

    <div class="overview-actions">
      <button type="button" class="overview-close" aria-label="Open launcher" onclick={() => sendAction({ type: "open-launcher" })}>
        <Icon name="search" />
      </button>
      <button type="button" class="overview-close" aria-label="Close overview" onclick={() => sendAction({ type: "toggle-overview" })}>
        <Icon name="close" />
      </button>
    </div>
  </header>

  <div class="overview-workspaces" onwheel={workspaceScroll}>
    {#each snapshot.workspaces as workspace (workspace.id)}
      <div
        class="workspace-preview"
        class:is-active={workspace.active}
        role="button"
        tabindex="0"
        aria-label={workspace.name}
        onclick={() => sendAction({ type: "workspace-switch", workspace: workspace.id })}
        onkeydown={(event) => workspaceKeydown(event, workspace)}
        ondragover={(event) => {
          if (!dragHasWindow(event)) return;
          event.preventDefault();
        }}
        ondrop={(event) => dropWindow(event, workspace)}
      >
        <div class="workspace-stage">
          {#each workspaceWindows(workspace.id) as window (window.id)}
            <button
              type="button"
              class="overview-window"
              class:is-active={window.active}
              draggable={true}
              style={geometryStyle(window)}
              onclick={(event) => {
                event.stopPropagation();
                sendAction({ type: "window-activate", window: window.id });
              }}
              ondragstart={(event) => windowDragStart(event, window)}
            >
              <strong>{window.title}</strong>
              <span>{window.appId ?? "App"}</span>
            </button>
          {/each}
        </div>
        <div class="workspace-label">
          <strong>{workspace.name}</strong>
          <span>{workspace.profile}</span>
        </div>
      </div>
    {/each}
  </div>

  <div class="overview-apps">
    {#each visibleApps as app, index (app.command)}
      <button
        type="button"
        class="overview-app"
        class:is-pinned={app.pinned}
        class:is-selected={clampedSelection >= 0 && index === clampedSelection}
        aria-label={app.name}
        onclick={() => launchApp(app)}
        oncontextmenu={(event) => pinApp(event, app)}
      >
        {#if app.iconUri}
          <img src={app.iconUri} alt="" />
        {:else}
          <Icon name="app" />
        {/if}
        <span>{app.name}</span>
      </button>
    {/each}
  </div>

  {#if snapshot.debugOverlay}
    <DebugMeter surface="OVERVIEW" />
  {/if}
</section>
