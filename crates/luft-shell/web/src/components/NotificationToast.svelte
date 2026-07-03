<script lang="ts">
  import Icon from "./Icon.svelte";
  import { sendAction } from "../shell/bridge";
  import { notificationTimeLabel } from "../lib/notifications";
  import type { NotificationItem, ShellSnapshot } from "../shell/model";

  let { snapshot }: { snapshot: ShellSnapshot } = $props();
  const item = $derived(snapshot.toastNotifications[0]);

  function notificationDefaultAction(notification: NotificationItem) {
    return notification.actions.find((action) => action.key === "default")?.key;
  }

  function visibleNotificationActions(notification: NotificationItem) {
    const defaultAction = notificationDefaultAction(notification);
    const actions = notification.actions
      .filter((action) => action.key !== "default")
      .slice(0, defaultAction ? 1 : 2);
    return defaultAction ? [{ key: defaultAction, label: "Open" }, ...actions] : actions;
  }

  function activate(notification: NotificationItem) {
    const action = notificationDefaultAction(notification);
    if (action) {
      sendAction({ type: "notification-action", notification: notification.id, action });
      return;
    }
    sendAction({ type: "toggle-date-center" });
  }

  function close(event: MouseEvent, notification: number) {
    event.stopPropagation();
    sendAction({ type: "notification-close", notification });
  }

  function notificationAction(event: MouseEvent, notification: NotificationItem, action: string) {
    event.stopPropagation();
    sendAction({ type: "notification-action", notification: notification.id, action });
  }
</script>

{#if item}
  <section class={`notification-toast is-${item.urgency}`}>
    <button
      type="button"
      class="toast-open"
      aria-label={notificationDefaultAction(item) ? `Open ${item.summary || item.appName || "notification"}` : "Open notification center"}
      onclick={() => activate(item)}
    >
      <span class="toast-avatar">
        {#if item.iconUri}
          <img src={item.iconUri} alt="" />
        {:else}
          <Icon name={item.urgency === "critical" ? "shield" : "bell"} />
        {/if}
      </span>

      <div class="toast-copy">
        <div class="toast-title-row">
          <span>{item.appName || "Application"}</span>
          <small>{item.urgency === "critical" ? `Critical / ${notificationTimeLabel(item.receivedAt)}` : notificationTimeLabel(item.receivedAt)}</small>
        </div>
        <strong>{item.summary || item.appName || "Notification"}</strong>
        {#if item.body}
          <p>{item.body}</p>
        {/if}
      </div>
    </button>

    <button type="button" class="toast-close" aria-label="Close notification" onclick={(event) => close(event, item.id)}>
      <Icon name="close" />
    </button>

    {#if visibleNotificationActions(item).length > 0}
      <div class="toast-actions">
        {#each visibleNotificationActions(item) as action (action.key)}
          <button type="button" onclick={(event) => notificationAction(event, item, action.key)}>{action.label}</button>
        {/each}
      </div>
    {/if}
  </section>
{/if}
