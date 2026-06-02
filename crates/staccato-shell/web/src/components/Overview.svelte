<script lang="ts">
  import DebugMeter from "./DebugMeter.svelte";
  import Icon from "./Icon.svelte";
  import { sendAction } from "../shell/bridge";
  import { filteredApplications, overviewSearchResults, selectedOverviewResult, type OverviewSearchResult } from "../lib/overview_state";
  import { geometryStyle } from "../lib/geometry";
  import type { Attachment } from "svelte/attachments";
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
  const searching = $derived(Boolean(query.trim()));
  const visibleApps = $derived(filteredApplications(snapshot, ""));
  const searchResults = $derived(overviewSearchResults(snapshot, query));
  const activeWindowCount = $derived.by(() => {
    if (!active) return 0;
    return workspaceWindows(active.id).length;
  });
  const clampedSelection = $derived.by(() => {
    if (selection < 0 || searchResults.length <= 0) return -1;
    return Math.max(0, Math.min(selection, searchResults.length - 1));
  });

  const focusSearch: Attachment<HTMLElement> = (node) => {
    requestAnimationFrame(() => node.focus());
  };

  function workspaceScroll(event: WheelEvent) {
    sendAction({ type: "workspace-relative", offset: event.deltaY > 0 ? 1 : -1 });
  }

  function scrollList(event: WheelEvent) {
    const target = event.currentTarget as HTMLElement;
    if (target.scrollHeight <= target.clientHeight) return;
    event.preventDefault();
    event.stopPropagation();
    target.scrollTop += event.deltaY;
  }

  function newWorkspace() {
    sendAction({ type: "workspace-new" });
  }

  function setSearch(value: string) {
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
    if (event.key === "Backspace") {
      event.preventDefault();
      setSearch(query.slice(0, -1));
      return;
    }
    if (event.key === "Delete") {
      event.preventDefault();
      setSearch("");
      return;
    }
    if (event.key === "Escape" && query) {
      event.preventDefault();
      event.stopPropagation();
      setSearch("");
      return;
    }
    if (event.key !== "Enter") {
      if (event.key.length === 1 && !event.altKey && !event.ctrlKey && !event.metaKey) {
        event.preventDefault();
        setSearch(`${query}${event.key}`);
      }
      return;
    }
    const result = selectedOverviewResult(searchResults, clampedSelection);
    if (!result) {
      if (!searching) {
        sendAction({ type: "open-launcher" });
      }
      return;
    }
    activateResult(result);
  }

  function searchPaste(event: ClipboardEvent) {
    const text = event.clipboardData?.getData("text/plain");
    if (!text) return;
    event.preventDefault();
    setSearch(`${query}${text.replace(/\s+/g, " ")}`);
  }

  function moveSelection(offset: number) {
    if (searchResults.length <= 0) return;
    const base = clampedSelection < 0 ? (offset > 0 ? -1 : 0) : clampedSelection;
    setSelection((base + offset + searchResults.length) % searchResults.length);
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

  function activateResult(result: OverviewSearchResult) {
    if (result.kind === "app") {
      sendAction({ type: "app-launch", command: result.app.command });
      return;
    }
    if (result.kind === "window") {
      sendAction({ type: "window-activate", window: result.window.id });
      return;
    }
    if (result.kind === "profile") {
      sendAction({ type: "workspace-set-profile", profile: result.profile.id });
      return;
    }
    if (result.kind === "action") {
      sendAction(result.action);
      return;
    }
    sendAction({ type: "workspace-switch", workspace: result.workspace.id });
  }
</script>

<section class="shell-overview">
  <header class="overview-top">
    <div class="overview-title">
      <strong>{active?.name ?? "Workspace"}</strong>
      <span>{activeWindowCount === 1 ? "1 window" : `${activeWindowCount} windows`} / {snapshot.time}</span>
    </div>

    <div
      {@attach focusSearch}
      class="overview-search"
      role="searchbox"
      tabindex="0"
      aria-label="Search apps"
      aria-placeholder="Search"
      onkeydown={searchKeydown}
      onpaste={searchPaste}
      onclick={(event) => (event.currentTarget as HTMLElement).focus()}
    >
      <Icon name="search" />
      <span class="overview-search-text">
        <span class="overview-search-value" class:is-placeholder={!query}>{query || "Search"}</span>
        <span class="overview-search-caret"></span>
      </span>
    </div>

    <div class="overview-actions">
      <button type="button" class="overview-close" aria-label="New workspace" onclick={newWorkspace}>
        <Icon name="plus" />
      </button>
      <button type="button" class="overview-close" aria-label="Close overview" onclick={() => sendAction({ type: "toggle-overview" })}>
        <Icon name="close" />
      </button>
    </div>
  </header>

  <div class="overview-workspaces" onwheel={workspaceScroll}>
    {#each snapshot.workspaces as workspace, workspaceIndex (workspace.id)}
      <div
        class="workspace-preview"
        class:is-active={workspace.active}
        style={`--index: ${workspaceIndex}`}
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
          {#each workspaceWindows(workspace.id) as window, windowIndex (window.id)}
            <button
              type="button"
              class="overview-window"
              class:is-active={window.active}
              draggable={true}
              style={`${geometryStyle(window)} --index: ${windowIndex}`}
              onclick={(event) => {
                event.stopPropagation();
                sendAction({ type: "window-activate", window: window.id });
              }}
              ondragstart={(event) => windowDragStart(event, window)}
            >
              <span class="overview-window-icon">
                {#if window.iconUri}
                  <img src={window.iconUri} alt="" />
                {:else}
                  <Icon name="app" />
                {/if}
              </span>
              <span class="overview-window-copy">
                <strong>{window.title}</strong>
                <span>{window.appId ?? "Window"}</span>
              </span>
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

  <div class:overview-apps={!searching} class:overview-results={searching} onwheel={scrollList}>
    {#if searching && searchResults.length === 0}
      <div class="overview-empty">
        <Icon name="search" />
        <span>No matches</span>
      </div>
    {:else if searching}
      {#each searchResults as result, index (result.key)}
        <button
          type="button"
          class="overview-result"
          class:is-selected={clampedSelection >= 0 && index === clampedSelection}
          style={`--index: ${index}`}
          onclick={() => activateResult(result)}
          onpointerenter={() => setSelection(index)}
        >
          <span class="overview-result-icon">
            {#if result.kind === "app" && result.iconUri}
              <img src={result.iconUri} alt="" />
            {:else if result.kind === "window" && result.iconUri}
              <img src={result.iconUri} alt="" />
            {:else if result.kind === "window"}
              <Icon name="browser" />
            {:else if result.kind === "workspace"}
              <Icon name="app" />
            {:else if result.kind === "profile"}
              <Icon name="settings" />
            {:else if result.kind === "action"}
              <Icon name={result.icon} />
            {:else}
              <Icon name="app" />
            {/if}
          </span>
          <span class="overview-result-copy">
            <strong>{result.title}</strong>
            <small>{result.detail}</small>
          </span>
          <span class="overview-result-kind">{result.kind === "action" ? result.label : result.kind}</span>
        </button>
      {/each}
    {:else if visibleApps.length === 0}
      <div class="overview-empty">
        <Icon name="search" />
        <span>No apps found</span>
      </div>
    {:else}
      {#each visibleApps as app, index (app.command)}
        <button
          type="button"
          class="overview-app"
          class:is-pinned={app.pinned}
          class:is-selected={clampedSelection >= 0 && index === clampedSelection}
          style={`--index: ${index}`}
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
    {/if}
  </div>

  {#if snapshot.debugOverlay}
    <DebugMeter surface="OVERVIEW" />
  {/if}
</section>
