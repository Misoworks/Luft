<script lang="ts">
  import { onDestroy } from "svelte";
  import AppIcon from "./AppIcon.svelte";
  import type { PanelApp } from "../shell/model";

  let {
    app,
    onlaunch,
    onmenu,
    onreorderstart,
    onreorderover,
    onreorderdrop,
    onreorderend,
    reorderable = true,
  }: {
    app: PanelApp;
    onlaunch: (app: PanelApp) => void;
    onmenu?: (command: string, x: number) => void;
    onreorderstart?: (command: string) => void;
    onreorderover?: (command: string, after: boolean) => void;
    onreorderdrop?: () => void;
    onreorderend?: () => void;
    reorderable?: boolean;
  } = $props();

  let launchRaised = $state(false);
  let jumping = $state(false);
  let settling = $state(false);
  let hovered = $state(false);
  let dragging = $state(false);
  let suppressClick = false;
  let reorderPointerId: number | undefined;
  let reorderStartX = 0;
  let reorderStartY = 0;
  let launchFrame: number | undefined;
  let launchTimer: ReturnType<typeof setTimeout> | undefined;
  let settleTimer: ReturnType<typeof setTimeout> | undefined;

  const className = "panel-app app-button";

  function openMenu(event: MouseEvent) {
    event.preventDefault();
    event.stopPropagation();
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    onmenu?.(app.command, Math.round(rect.left + rect.width / 2));
  }

  function launch(event: MouseEvent) {
    if (suppressClick) {
      event.preventDefault();
      suppressClick = false;
      return;
    }
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
    onlaunch(app);
  }

  const runningDots = $derived(
    Array.from({
      length: Math.min(4, Math.max(app.windowIds.length, app.running ? 1 : 0)),
    }),
  );

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

  function pointerDown(event: PointerEvent) {
    if (!reorderable || !onreorderstart || event.button !== 0) return;
    reorderPointerId = event.pointerId;
    reorderStartX = event.clientX;
    reorderStartY = event.clientY;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  }

  function pointerMove(event: PointerEvent) {
    if (event.pointerId !== reorderPointerId) return;
    const moved = Math.hypot(event.clientX - reorderStartX, event.clientY - reorderStartY);
    if (!dragging && moved < 8) return;
    if (!dragging) {
      dragging = true;
      suppressClick = true;
      onreorderstart?.(app.command);
    }
    previewPointerTarget(event.clientX, event.clientY);
  }

  function pointerUp(event: PointerEvent) {
    if (event.pointerId !== reorderPointerId) return;
    releasePointer(event);
    if (dragging) {
      previewPointerTarget(event.clientX, event.clientY);
      onreorderdrop?.();
      dragging = false;
      window.setTimeout(() => {
        suppressClick = false;
      }, 0);
      return;
    }
    reorderPointerId = undefined;
  }

  function pointerCancel(event: PointerEvent) {
    if (event.pointerId !== reorderPointerId) return;
    releasePointer(event);
    if (!dragging) {
      reorderPointerId = undefined;
      return;
    }
    dragging = false;
    onreorderend?.();
    window.setTimeout(() => {
      suppressClick = false;
    }, 0);
  }

  function releasePointer(event: PointerEvent) {
    const target = event.currentTarget as HTMLElement;
    if (target.hasPointerCapture(event.pointerId)) {
      target.releasePointerCapture(event.pointerId);
    }
    reorderPointerId = undefined;
  }

  function previewPointerTarget(clientX: number, clientY: number) {
    if (!onreorderover) return;
    const target = document.elementFromPoint(clientX, clientY)?.closest<HTMLElement>(".app-button");
    const command = target?.dataset.command;
    if (!command) return;
    const rect = target.getBoundingClientRect();
    onreorderover(command, clientX > rect.left + rect.width / 2);
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
  class:is-reordering={dragging}
  data-command={app.command}
  aria-label={app.label}
  onclick={launch}
  oncontextmenu={openMenu}
  onpointerdown={pointerDown}
  onpointermove={pointerMove}
  onpointerup={pointerUp}
  onpointercancel={pointerCancel}
  onpointerenter={pointerEnter}
  onpointerleave={pointerLeave}
>
  <AppIcon {app} />
  <span class="running-dots" aria-hidden="true">
    {#each runningDots as _, index (`${app.command}-${index}`)}
      <span class="running-dot" class:is-active-dot={app.active && index === 0}></span>
    {/each}
  </span>
</button>
