<script lang="ts">
  import AppIcon from "./AppIcon.svelte";
  import type { DockApp } from "../shell/model";

  type Variant = "dock" | "taskbar";

  let {
    app,
    variant,
    launching = false,
    hoverEpoch = 0,
    onlaunch,
    onmenu,
  }: {
    app: DockApp;
    variant: Variant;
    launching?: boolean;
    hoverEpoch?: number;
    onlaunch: (command: string) => void;
    onmenu?: (command: string) => void;
  } = $props();

  let hovered = $state(false);
  let seenHoverEpoch = $state(-1);
  let launchRaised = $state(false);
  let jumping = $state(false);
  let launchTimer: ReturnType<typeof setTimeout> | undefined;

  const className = $derived(`${variant === "dock" ? "dock-item" : "taskbar-app"} app-button`);
  const externallyHovered = $derived(hovered && seenHoverEpoch === hoverEpoch);

  function openMenu(event: MouseEvent) {
    event.preventDefault();
    onmenu?.(app.command);
  }

  function launch() {
    launchRaised = externallyHovered;
    jumping = false;
    requestAnimationFrame(() => {
      jumping = true;
    });
    if (launchTimer) {
      clearTimeout(launchTimer);
    }
    launchTimer = setTimeout(() => {
      jumping = false;
      launchRaised = false;
    }, 430);
    onlaunch(app.command);
  }

  function pointerEntered() {
    seenHoverEpoch = hoverEpoch;
    hovered = true;
  }

  function pointerLeft() {
    hovered = false;
  }
</script>

<button
  type="button"
  class={className}
  class:is-active={app.active}
  class:is-running={app.running}
  class:is-launching={launching || jumping}
  class:is-hovered={externallyHovered}
  class:is-launch-raised={launchRaised}
  data-command={app.command}
  aria-label={app.label}
  onclick={launch}
  oncontextmenu={openMenu}
  onpointerenter={pointerEntered}
  onpointermove={pointerEntered}
  onpointerleave={pointerLeft}
  onpointercancel={pointerLeft}
  onlostpointercapture={pointerLeft}
  onmouseleave={pointerLeft}
  onblur={pointerLeft}
>
  <AppIcon {app} />
  <span class="running-dot"></span>
</button>
