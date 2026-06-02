<script lang="ts">
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
    onmenu?: (command: string) => void;
  } = $props();

  let launchRaised = $state(false);
  let jumping = $state(false);
  let launchTimer: ReturnType<typeof setTimeout> | undefined;

  const className = $derived(`${variant === "dock" ? "dock-item" : "taskbar-app"} app-button`);

  function openMenu(event: MouseEvent) {
    event.preventDefault();
    onmenu?.(app.command);
  }

  function launch(event: MouseEvent) {
    launchRaised = (event.currentTarget as HTMLElement).matches(":hover");
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
</script>

<button
  type="button"
  class={className}
  class:is-active={app.active}
  class:is-running={app.running}
  class:is-launching={jumping}
  class:is-launch-raised={launchRaised}
  data-command={app.command}
  aria-label={app.label}
  onclick={launch}
  oncontextmenu={openMenu}
>
  <AppIcon {app} />
  <span class="running-dot"></span>
</button>
