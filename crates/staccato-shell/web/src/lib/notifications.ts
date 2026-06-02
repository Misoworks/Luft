import type { NotificationItem } from "../shell/model";

export type NotificationGroup = {
  key: string;
  appName: string;
  iconUri?: string;
  urgency: NotificationItem["urgency"];
  receivedAt: number;
  items: NotificationItem[];
};

export function notificationGroups(items: NotificationItem[]) {
  const groups: NotificationGroup[] = [];
  const byKey = new Map<string, NotificationGroup>();

  for (const item of items) {
    const key = notificationKey(item);
    let group = byKey.get(key);
    if (!group) {
      group = {
        key,
        appName: item.appName || "Application",
        iconUri: item.iconUri,
        urgency: item.urgency,
        receivedAt: item.receivedAt,
        items: [],
      };
      byKey.set(key, group);
      groups.push(group);
    }
    group.items.push(item);
    group.urgency = strongestUrgency(group.urgency, item.urgency);
    group.iconUri ??= item.iconUri;
    group.receivedAt = Math.max(group.receivedAt, item.receivedAt);
  }

  return groups;
}

export function notificationTimeLabel(receivedAt: number) {
  const seconds = Math.max(0, Math.floor(Date.now() / 1000 - receivedAt));
  if (seconds < 10) return "Now";
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h`;
  return `${Math.floor(hours / 24)}d`;
}

function notificationKey(item: NotificationItem) {
  return (item.appName || "Application").trim().toLocaleLowerCase();
}

function strongestUrgency(left: NotificationItem["urgency"], right: NotificationItem["urgency"]) {
  if (left === "critical" || right === "critical") return "critical";
  if (left === "normal" || right === "normal") return "normal";
  return "low";
}
