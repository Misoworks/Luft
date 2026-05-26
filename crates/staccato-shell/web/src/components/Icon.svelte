<script lang="ts">
  import { iconNode } from "../shell/icons";

  let { name }: { name: string } = $props();
  const nodes = $derived(iconNode(name));
</script>

<svg aria-hidden="true" width="24" height="24" viewBox="0 0 24 24">
  {#each nodes as node, index (`${node[0]}-${index}`)}
    {#if node[0] === "path"}
      <path {...node[1]} />
    {:else if node[0] === "circle"}
      <circle {...node[1]} />
    {:else if node[0] === "rect"}
      <rect {...node[1]} />
    {:else if node[0] === "line"}
      <line {...node[1]} />
    {:else if node[0] === "polyline"}
      <polyline {...node[1]} />
    {:else if node[0] === "polygon"}
      <polygon {...node[1]} />
    {:else if node[0] === "ellipse"}
      <ellipse {...node[1]} />
    {/if}
  {/each}
</svg>
