export function movePanelCommand(
  order: string[],
  dragged: string,
  target: string,
  after: boolean,
) {
  if (dragged === target) return order;
  const next = order.filter((command) => command !== dragged);
  const targetIndex = next.indexOf(target);
  if (targetIndex < 0) return order;
  next.splice(targetIndex + (after ? 1 : 0), 0, dragged);
  return sameOrder(order, next) ? order : next;
}

export function sameOrder(left: string[], right: string[]) {
  return left.length === right.length && left.every((command, index) => command === right[index]);
}
