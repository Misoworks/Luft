import type { WindowItem } from "../shell/model";

export function geometryStyle(window: WindowItem) {
  const left = clamp((window.geometry.x / 1280) * 100, 4, 82);
  const top = clamp((window.geometry.y / 800) * 100, 8, 72);
  const width = clamp((window.geometry.width / 1280) * 100, 18, 76);
  const height = clamp((window.geometry.height / 800) * 100, 16, 70);
  return `left:${left}%;top:${top}%;width:${width}%;height:${height}%;`;
}

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}
