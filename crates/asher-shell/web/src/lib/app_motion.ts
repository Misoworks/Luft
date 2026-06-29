import type { TransitionConfig } from "svelte/transition";
import { backOut, cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";

export function animationsEnabled() {
  return document.querySelector<HTMLElement>("#app")?.dataset.animations === "true";
}

export function appFly(
  node: Element,
  params: { y: number; duration: number; easing: (value: number) => number; opacity?: number },
): TransitionConfig {
  if (!animationsEnabled()) {
    return { duration: 0 };
  }

  return fly(node, {
    ...params,
    opacity: params.opacity ?? 1,
  });
}

export const runningAppEnter = {
  y: 14,
  duration: 240,
  easing: backOut,
  opacity: 1,
};

export const runningAppExit = {
  y: 8,
  duration: 160,
  easing: cubicOut,
  opacity: 1,
};
