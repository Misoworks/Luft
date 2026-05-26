<script lang="ts">
  import { sendAction } from "../shell/bridge";
  import type { DockApp, ShellSnapshot } from "../shell/model";

  let { snapshot }: { snapshot: ShellSnapshot } = $props();
  const app = $derived(snapshot.dockApps.find((entry) => entry.command === snapshot.dockMenuCommand));

  function close() {
    sendAction({ type: "dock-menu-close" });
  }

  function open(app: DockApp) {
    close();
    sendAction({ type: "dock-launch", command: app.command });
  }

  function unpin(app: DockApp) {
    close();
    sendAction({ type: "dock-unpin", command: app.command });
  }
</script>

<section class="dock-menu-shell">
  {#if app}
    <div class="dock-menu" role="menu" tabindex="-1" data-command={app.command} onpointerdown={(event) => event.stopPropagation()}>
      <strong>{app.label}</strong>
      <button type="button" class="dock-menu-item" role="menuitem" onclick={() => open(app)}>
        <span>Open</span>
      </button>
      <button type="button" class="dock-menu-item" role="menuitem" onclick={() => unpin(app)}>
        <span>Unpin from Dock</span>
      </button>
    </div>
  {/if}
</section>
