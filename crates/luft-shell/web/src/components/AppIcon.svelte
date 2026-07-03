<script lang="ts">
  import Icon from "./Icon.svelte";
  import { appIconSignature } from "../lib/app_icons";
  import type { PanelApp } from "../shell/model";

  let { app }: { app: PanelApp } = $props();
</script>

<span class="app-icon" data-icon={appIconSignature(app)}>
  {#if app.iconUri}
    <img src={app.iconUri} alt="" />
  {:else}
    <Icon name="app" />
  {/if}
</span>
