<script lang="ts">
  import { onDestroy } from "svelte";
  import AppIcon from "./AppIcon.svelte";
  import type { DockApp } from "../shell/model";

  type Variant = "dock" | "taskbar";

  let {
    app,
    variant,
    onlaunch,
    onmenu,
  }: {
    app: DockApp;
    variant: Variant;
    onlaunch: (command: string) => void;
    onmenu?: (command: string, x: number) => void;
  } = $props();

  let launchRaised = $state(false);
  let jumping = $state(false);
  let settling = $state(false);
  let hovered = $state(false);
  let launchFrame: number | undefined;
  let launchTimer: ReturnType<typeof setTimeout> | undefined;
  let settleTimer: ReturnType<typeof setTimeout> | undefined;

  const className = $derived(`${variant === "dock" ? "dock-item" : "taskbar-app"} app-button`);

  function openMenu(event: MouseEvent) {
    event.preventDefault();
    event.stopPropagation();
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    onmenu?.(app.command, Math.round(rect.left + rect.width / 2));
  }

  function launch(event: MouseEvent) {
    hovered = (event.currentTarget as HTMLElement).matches(":hover");
    launchRaised = hovered;
    jumping = false;
    settling = false;
    if (launchFrame !== undefined) {
      cancelAnimationFrame(launchFrame);
    }
    if (settleTimer) {
      clearTimeout(settleTimer);
    }
    launchFrame = requestAnimationFrame(() => {
      launchFrame = undefined;
      jumping = true;
    });
    if (launchTimer) {
      clearTimeout(launchTimer);
    }
    launchTimer = setTimeout(() => {
      const shouldSettle = launchRaised && !hovered;
      jumping = false;
      launchRaised = false;
      if (shouldSettle) {
        settling = true;
        settleTimer = setTimeout(() => {
          settling = false;
        }, 180);
      }
    }, 430);
    onlaunch(app.command);
  }

  function pointerEnter() {
    hovered = true;
    if (settling) {
      settling = false;
    }
    if (settleTimer) {
      clearTimeout(settleTimer);
    }
  }

  function pointerLeave() {
    hovered = false;
  }

  onDestroy(() => {
    if (launchFrame !== undefined) {
      cancelAnimationFrame(launchFrame);
    }
    if (launchTimer) {
      clearTimeout(launchTimer);
    }
    if (settleTimer) {
      clearTimeout(settleTimer);
    }
  });
</script>

<button
  type="button"
  class={className}
  class:is-active={app.active}
  class:is-running={app.running}
  class:is-launching={jumping}
  class:is-launch-raised={launchRaised}
  class:is-launch-settling={settling}
  data-command={app.command}
  aria-label={app.label}
  onclick={launch}
  oncontextmenu={openMenu}
  onpointerenter={pointerEnter}
  onpointerleave={pointerLeave}
>
  <AppIcon {app} />
  <span class="running-dot"></span>
</button>
