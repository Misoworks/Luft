import type { TransitionConfig } from "svelte/transition";
import { cubicOut } from "svelte/easing";

export function appFly(
  node: Element,
  params: { y: number; duration: number; easing: (value: number) => number; opacity?: number },
): TransitionConfig {
  const y = params.y;
  const opacity = params.opacity ?? 0;
  return {
    duration: params.duration,
    easing: params.easing,
    css: (t) => {
      const offset = (1 - t) * y;
      const alpha = opacity + (1 - opacity) * t;
      return `opacity: ${alpha}; transform: translate3d(0, ${offset}px, 0) scale(${0.92 + t * 0.08});`;
    },
  };
}

export const runningAppEnter = {
  y: 10,
  duration: 170,
  easing: cubicOut,
  opacity: 0,
};

export const runningAppExit = {
  y: 8,
  duration: 160,
  easing: cubicOut,
  opacity: 1,
};
