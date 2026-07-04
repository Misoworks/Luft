<script lang="ts">
  import Icon from "./Icon.svelte";
  import { sendAction } from "../shell/bridge";
  import type { ShellSnapshot } from "../shell/model";

  let { snapshot }: { snapshot: ShellSnapshot } = $props();

  function close() {
    sendAction({ type: "session-menu-close" });
  }

  function run(command: "lock" | "suspend" | "reboot" | "power-off") {
    close();
    sendAction({ type: "session-command", command });
  }
</script>

<section class="session-menu-shell">
  <div class="session-menu" role="menu" tabindex="-1" onpointerdown={(event) => event.stopPropagation()}>
    <button type="button" class="session-menu-item" role="menuitem" onclick={() => run("lock")}>
      <span class="session-menu-icon"><Icon name="lock" /></span>
      <span>Lock</span>
    </button>
    <button type="button" class="session-menu-item" role="menuitem" onclick={() => run("suspend")}>
      <span class="session-menu-icon"><Icon name="moon" /></span>
      <span>Suspend</span>
    </button>
    <button type="button" class="session-menu-item" role="menuitem" onclick={() => run("reboot")}>
      <span class="session-menu-icon"><Icon name="refresh" /></span>
      <span>Restart</span>
    </button>
    <button type="button" class="session-menu-item is-danger" role="menuitem" onclick={() => run("power-off")}>
      <span class="session-menu-icon"><Icon name="power" /></span>
      <span>Power Off</span>
    </button>
  </div>
</section>
