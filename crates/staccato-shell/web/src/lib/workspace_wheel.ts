export function workspaceWheelOffset(event: WheelEvent) {
  const delta = Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.deltaY;
  if (Math.abs(delta) < 1) return 0;
  event.preventDefault();
  event.stopPropagation();
  return delta > 0 ? 1 : -1;
}
