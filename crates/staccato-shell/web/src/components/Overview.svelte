<script lang="ts">
  import DebugMeter from "./DebugMeter.svelte";
  import Icon from "./Icon.svelte";
  import { sendAction } from "../shell/bridge";
  import { filteredApplications, overviewSearchResults, selectedOverviewResult, type OverviewSearchResult } from "../lib/overview_state";
  import type { Attachment } from "svelte/attachments";
  import type { ApplicationItem, ShellSnapshot } from "../shell/model";

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

  const APPS_PER_PAGE = 24;
  const PAGE_WHEEL_DELAY = 280;

  let appPage = $state(0);
  let lastPageWheelAt = 0;

  const searching = $derived(Boolean(query.trim()));
  const visibleApps = $derived(filteredApplications(snapshot, ""));
  const appPages = $derived.by(() => paginateApplications(visibleApps));
  const appPageCount = $derived(Math.max(1, appPages.length));
  const currentAppPage = $derived(Math.min(appPage, appPageCount - 1));
  const searchResults = $derived(overviewSearchResults(snapshot, query));
  const clampedSelection = $derived.by(() => {
    if (selection < 0 || searchResults.length <= 0) return -1;
    return Math.max(0, Math.min(selection, searchResults.length - 1));
  });

  const focusSearch: Attachment<HTMLInputElement> = (node) => {
    requestAnimationFrame(() => node.focus({ preventScroll: true }));
  };

  function scrollOverviewList(event: WheelEvent) {
    const node = event.currentTarget as HTMLElement;
    const maxScroll = node.scrollHeight - node.clientHeight;
    if (maxScroll <= 0) return;
    const next = Math.max(0, Math.min(maxScroll, node.scrollTop + event.deltaY));
    if (next === node.scrollTop) return;
    event.preventDefault();
    node.scrollTop = next;
  }

  function overviewAppsWheel(event: WheelEvent) {
    if (searching) {
      scrollOverviewList(event);
      return;
    }
    if (appPageCount <= 1) return;
    const delta = Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.deltaY;
    if (Math.abs(delta) < 10) return;
    const now = performance.now();
    if (now - lastPageWheelAt < PAGE_WHEEL_DELAY) return;
    lastPageWheelAt = now;
    event.preventDefault();
    setAppPage(currentAppPage + (delta > 0 ? 1 : -1));
  }

  function setSearch(value: string) {
    setQuery(value);
    setSelection(value.trim() ? 0 : -1);
    if (value.trim()) {
      setAppPage(0);
    }
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
    if (event.key === "Escape" && query) {
      event.preventDefault();
      event.stopPropagation();
      setSearch("");
      return;
    }
    if (event.key !== "Enter") {
      return;
    }
    event.preventDefault();
    const result = selectedOverviewResult(searchResults, clampedSelection);
    if (!result) {
      if (!searching) {
        sendAction({ type: "open-launcher" });
      }
      return;
    }
    activateResult(result);
  }

  function searchInput(event: Event) {
    setSearch((event.currentTarget as HTMLInputElement).value);
  }

  function moveSelection(offset: number) {
    if (searchResults.length <= 0) return;
    const base = clampedSelection < 0 ? (offset > 0 ? -1 : 0) : clampedSelection;
    setSelection((base + offset + searchResults.length) % searchResults.length);
  }

  function launchApp(app: ApplicationItem) {
    sendAction({ type: "app-launch", command: app.command });
  }

  function paginateApplications(apps: ApplicationItem[]) {
    const pages: ApplicationItem[][] = [];
    for (let start = 0; start < apps.length; start += APPS_PER_PAGE) {
      pages.push(apps.slice(start, start + APPS_PER_PAGE));
    }
    return pages;
  }

  function appPageKey(page: ApplicationItem[], index: number) {
    return page.map((app) => app.command).join("|") || `empty-${index}`;
  }

  function pageRows(page: ApplicationItem[], columns: number) {
    return Math.max(1, Math.ceil(page.length / columns));
  }

  function setAppPage(page: number) {
    appPage = Math.max(0, Math.min(page, appPageCount - 1));
  }

  function previousAppPage() {
    setAppPage(currentAppPage - 1);
  }

  function nextAppPage() {
    setAppPage(currentAppPage + 1);
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
    <label class="overview-search">
      <Icon name="search" />
      <input
        {@attach focusSearch}
        class="overview-search-input"
        type="text"
        aria-label="Search apps"
        inputmode="search"
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
        placeholder="Search"
        value={query}
        oninput={searchInput}
        onkeydown={searchKeydown}
      />
    </label>
  </header>

  <div class:overview-apps={!searching} class:overview-results={searching} onwheel={overviewAppsWheel}>
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
      <div class="overview-app-pages" style={`--page-offset: -${currentAppPage * 100}%`}>
        {#each appPages as page, pageIndex (appPageKey(page, pageIndex))}
          <div
            class="overview-app-page"
            style:--page-rows={pageRows(page, 8)}
            style:--mobile-page-rows={pageRows(page, 3)}
            aria-hidden={pageIndex !== currentAppPage}
          >
            {#each page as app, appIndex (app.command)}
              <button
                type="button"
                class="overview-app"
                class:is-pinned={app.pinned}
                style={`--index: ${pageIndex * APPS_PER_PAGE + appIndex}`}
                aria-label={app.name}
                tabindex={pageIndex === currentAppPage ? 0 : -1}
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
        {/each}
      </div>
    {/if}
  </div>

  {#if !searching && appPages.length > 1}
    <nav class="overview-pagination" aria-label="Application pages">
      <button type="button" class="overview-page-control" aria-label="Previous app page" onclick={previousAppPage} disabled={currentAppPage === 0}>
        <Icon name="chevron-left" />
      </button>
      <div class="overview-page-dots">
        {#each appPages as page, pageIndex (appPageKey(page, pageIndex))}
          <button
            type="button"
            class="overview-page-dot"
            class:is-active={pageIndex === currentAppPage}
            aria-label={`Application page ${pageIndex + 1}`}
            aria-current={pageIndex === currentAppPage ? "page" : undefined}
            onclick={() => setAppPage(pageIndex)}
          >
            <span></span>
          </button>
        {/each}
      </div>
      <button
        type="button"
        class="overview-page-control"
        aria-label="Next app page"
        onclick={nextAppPage}
        disabled={currentAppPage === appPages.length - 1}
      >
        <Icon name="chevron-right" />
      </button>
    </nav>
  {/if}

  {#if snapshot.debugOverlay}
    <DebugMeter surface="OVERVIEW" />
  {/if}
</section>
