<script lang="ts">
  import Icon from "./Icon.svelte";

  let {
    label,
    icon,
    value,
    available = true,
    muted = false,
    index = 0,
    onChange,
    onToggle,
  }: {
    label: string;
    icon: string;
    value: number;
    available?: boolean;
    muted?: boolean;
    index?: number;
    onChange: (value: number) => void;
    onToggle?: () => void;
  } = $props();

  let dragging = $state(false);
  let draftValue = $state<number | null>(null);

  const displayValue = $derived(clamp(draftValue ?? value));
  const currentIcon = $derived(muted ? "volume-muted" : icon);

  function preview(value: number) {
    if (!available) return;
    draftValue = clamp(value);
  }

  function commit(value = displayValue) {
    if (!available) return;
    const next = clamp(value);
    draftValue = next;
    onChange(next);
    window.setTimeout(() => {
      if (!dragging) draftValue = null;
    }, 180);
  }

  function valueFromPointer(event: PointerEvent) {
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    return ((event.clientX - rect.left) / rect.width) * 100;
  }

  function pointerdown(event: PointerEvent) {
    if (!available) return;
    event.preventDefault();
    const target = event.currentTarget as HTMLElement;
    target.setPointerCapture(event.pointerId);
    dragging = true;
    preview(valueFromPointer(event));
  }

  function pointermove(event: PointerEvent) {
    if (!dragging) return;
    preview(valueFromPointer(event));
  }

  function pointerup(event: PointerEvent) {
    if (!dragging) return;
    const target = event.currentTarget as HTMLElement;
    if (target.hasPointerCapture(event.pointerId)) {
      target.releasePointerCapture(event.pointerId);
    }
    dragging = false;
    commit();
  }

  function keydown(event: KeyboardEvent) {
    if (!available) return;
    if (event.key === "ArrowLeft" || event.key === "ArrowDown") {
      event.preventDefault();
      commit(displayValue - 5);
    } else if (event.key === "ArrowRight" || event.key === "ArrowUp") {
      event.preventDefault();
      commit(displayValue + 5);
    } else if (event.key === "Home") {
      event.preventDefault();
      commit(0);
    } else if (event.key === "End") {
      event.preventDefault();
      commit(100);
    }
  }

  function toggle() {
    if (available) onToggle?.();
  }

  function clamp(value: number) {
    return Math.round(Math.min(100, Math.max(0, value)));
  }
</script>

<section
  class="control-slider"
  class:is-disabled={!available}
  class:is-dragging={dragging}
  style:--slider-ratio={`${available ? displayValue / 100 : 0}`}
  style:--index={index}
>
  <button type="button" class="control-slider-icon" aria-label={onToggle ? `Toggle ${label}` : label} onclick={toggle} disabled={!available}>
    <Icon name={currentIcon} />
  </button>
  <div class="control-slider-body">
    <div class="control-slider-copy">
      <span>{label}</span>
      <strong>{available ? `${displayValue}%` : "Unavailable"}</strong>
    </div>
    <button
      type="button"
      class="control-slider-track"
      role="slider"
      aria-label={label}
      aria-valuemin="0"
      aria-valuemax="100"
      aria-valuenow={available ? displayValue : undefined}
      aria-disabled={!available}
      onpointerdown={pointerdown}
      onpointermove={pointermove}
      onpointerup={pointerup}
      onpointercancel={pointerup}
      onkeydown={keydown}
      disabled={!available}
    >
      <span class="control-slider-fill"></span>
      <span class="control-slider-thumb"></span>
    </button>
  </div>
</section>
