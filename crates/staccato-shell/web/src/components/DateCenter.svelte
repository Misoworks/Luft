<script lang="ts">
  import DebugMeter from "./DebugMeter.svelte";
  import Icon from "./Icon.svelte";
  import { sendAction } from "../shell/bridge";
  import { calendarCells, sameDay } from "../lib/calendar";
  import { notificationGroups, notificationTimeLabel, type NotificationGroup } from "../lib/notifications";
  import type { NotificationItem, ShellSnapshot } from "../shell/model";

  const DISMISS_DELAY = 120;

  let { snapshot }: { snapshot: ShellSnapshot } = $props();
  let calendarMonthOffset = $state(0);
  let selectedDay = $state<string | undefined>();
  let dismissingNotifications = $state<number[]>([]);
  let expandedGroups = $state<string[]>([]);

  const clockKey = $derived(`${snapshot.date} ${snapshot.time}`);
  const today = $derived.by(() => {
    clockKey;
    return new Date();
  });
  const month = $derived(new Date(today.getFullYear(), today.getMonth() + calendarMonthOffset, 1));
  const cells = $derived(calendarCells(month));
  const notificationCount = $derived(snapshot.notifications.length);
  const groups = $derived(notificationGroups(snapshot.notifications));
  const monthLabel = $derived(month.toLocaleDateString([], { month: "long", year: "numeric" }));
  const todayLabel = $derived(today.toLocaleDateString([], { weekday: "long" }));
  const todayFull = $derived(today.toLocaleDateString([], { month: "long", day: "numeric", year: "numeric" }));
  const activeDay = $derived(selectedDay ?? calendarKey(today));

  function scrollMonth(event: WheelEvent) {
    event.preventDefault();
    calendarMonthOffset += event.deltaY > 0 ? 1 : -1;
  }

  function previousMonth() {
    calendarMonthOffset -= 1;
  }

  function nextMonth() {
    calendarMonthOffset += 1;
  }

  function resetMonth() {
    calendarMonthOffset = 0;
  }

  function selectDay(date: Date) {
    selectedDay = calendarKey(date);
    calendarMonthOffset = (date.getFullYear() - today.getFullYear()) * 12 + date.getMonth() - today.getMonth();
  }

  function calendarKey(date: Date) {
    return `${date.getFullYear()}-${date.getMonth()}-${date.getDate()}`;
  }

  function calendarLabel(date: Date) {
    return date.toLocaleDateString([], {
      weekday: "long",
      month: "long",
      day: "numeric",
      year: "numeric",
    });
  }

  function calendarCurrentKeydown(event: KeyboardEvent) {
    if (event.key === "ArrowLeft" || event.key === "PageUp") {
      event.preventDefault();
      previousMonth();
      return;
    }
    if (event.key === "ArrowRight" || event.key === "PageDown") {
      event.preventDefault();
      nextMonth();
      return;
    }
    if (event.key === "Home") {
      event.preventDefault();
      resetMonth();
    }
  }

  function closeNotification(notification: number) {
    dismissNotifications([notification], () => sendAction({ type: "notification-close", notification }));
  }

  function clearAllNotifications() {
    dismissNotifications(
      snapshot.notifications.map((notification) => notification.id),
      () => sendAction({ type: "notification-clear-all" })
    );
  }

  function clearGroup(group: NotificationGroup) {
    dismissNotifications(
      group.items.map((notification) => notification.id),
      () => {
        for (const notification of group.items) {
          sendAction({ type: "notification-close", notification: notification.id });
        }
      }
    );
  }

  function toggleDoNotDisturb() {
    sendAction({ type: "notification-do-not-disturb", enabled: !snapshot.doNotDisturb });
  }

  function notificationAction(notification: number, action: string) {
    dismissNotifications([notification], () => sendAction({ type: "notification-action", notification, action }));
  }

  function notificationDefaultAction(notification: NotificationItem) {
    return notification.actions.find((action) => action.key === "default")?.key;
  }

  function visibleNotificationActions(notification: NotificationItem) {
    return notification.actions.filter((action) => action.key !== "default").slice(0, 3);
  }

  function openNotification(notification: NotificationItem) {
    const action = notificationDefaultAction(notification);
    if (!action) return;
    notificationAction(notification.id, action);
  }

  function groupCountLabel(group: NotificationGroup) {
    return group.items.length === 1 ? "1 notification" : `${group.items.length} notifications`;
  }

  function visibleItems(group: NotificationGroup) {
    return groupExpanded(group) ? group.items : group.items.slice(0, 1);
  }

  function hiddenCount(group: NotificationGroup) {
    return Math.max(0, group.items.length - visibleItems(group).length);
  }

  function toggleGroup(group: NotificationGroup) {
    if (group.items.length <= 1) return;
    if (expandedGroups.includes(group.key)) {
      expandedGroups = expandedGroups.filter((key) => key !== group.key);
      return;
    }
    expandedGroups = [...expandedGroups, group.key];
  }

  function dismissNotifications(ids: number[], after: () => void) {
    const next = ids.filter((id) => !dismissingNotifications.includes(id));
    if (next.length === 0) return;
    dismissingNotifications = [...dismissingNotifications, ...next];
    window.setTimeout(() => {
      after();
      dismissingNotifications = dismissingNotifications.filter((id) => !next.includes(id));
    }, DISMISS_DELAY);
  }

  function isDismissing(notification: number) {
    return dismissingNotifications.includes(notification);
  }

  function groupDismissing(group: NotificationGroup) {
    return group.items.every((notification) => isDismissing(notification.id));
  }

  function groupExpanded(group: NotificationGroup) {
    return group.items.length <= 1 || expandedGroups.includes(group.key);
  }
</script>

<section class="popover date-center" class:is-empty={groups.length === 0}>
  <section class="notification-center">
    <header class="notification-header">
      <div>
        <span>Notifications</span>
        <small>{notificationCount === 1 ? "1 notification" : `${notificationCount} notifications`}</small>
      </div>
      <div class="notification-controls">
        {#if notificationCount > 0}
          <button type="button" class="notification-control" onclick={clearAllNotifications}>Clear</button>
        {/if}
        <button
          type="button"
          class="notification-control is-icon"
          class:is-on={snapshot.doNotDisturb}
          aria-label={snapshot.doNotDisturb ? "Disable do not disturb" : "Enable do not disturb"}
          onclick={toggleDoNotDisturb}
        >
          <Icon name={snapshot.doNotDisturb ? "bell-off" : "bell"} />
        </button>
      </div>
    </header>

    <div class="notifications" aria-live="polite">
      {#if groups.length === 0}
        <div class="notification-empty" class:is-muted={snapshot.doNotDisturb}>
          <Icon name={snapshot.doNotDisturb ? "bell-off" : "bell"} />
          <span>{snapshot.doNotDisturb ? "Do Not Disturb" : "No Notifications"}</span>
        </div>
      {:else}
        {#each groups as group, index (group.key)}
          <section
            class={`notification-group is-${group.urgency}`}
            class:is-dismissing={groupDismissing(group)}
            class:is-expanded={groupExpanded(group)}
            style={`--index: ${index}`}
          >
            <header class="notification-group-header">
              <span class="notification-avatar">
                {#if group.iconUri}
                  <img src={group.iconUri} alt="" />
                {:else}
                  <Icon name={group.urgency === "critical" ? "shield" : "bell"} />
                {/if}
              </span>
              <div class="notification-group-copy">
                <strong>{group.appName}</strong>
                <small>{groupCountLabel(group)} / {notificationTimeLabel(group.receivedAt)}</small>
              </div>
              <div class="notification-group-actions">
                {#if group.items.length > 1}
                  <button
                    type="button"
                    class="notification-expand"
                    class:is-expanded={groupExpanded(group)}
                    aria-expanded={groupExpanded(group)}
                    aria-label={groupExpanded(group) ? `Collapse ${group.appName}` : `Expand ${group.appName}`}
                    onclick={() => toggleGroup(group)}
                  >
                    <span>{groupExpanded(group) ? "Less" : `${hiddenCount(group)} more`}</span>
                    <Icon name="chevron-down" />
                  </button>
                {/if}
                <button type="button" class="icon-button" aria-label={`Clear ${group.appName} notifications`} onclick={() => clearGroup(group)}>
                  <Icon name="close" />
                </button>
              </div>
            </header>
            <div class="notification-stack" class:is-collapsed={!groupExpanded(group)}>
              {#each visibleItems(group) as item (item.id)}
                <article
                  class={`notification is-${item.urgency}`}
                  class:is-actionable={Boolean(notificationDefaultAction(item))}
                  class:is-dismissing={isDismissing(item.id)}
                >
                  {#if notificationDefaultAction(item)}
                    <button type="button" class="notification-body notification-main" onclick={() => openNotification(item)}>
                      <div class="notification-title-row">
                        <strong>{item.summary}</strong>
                        <small>{notificationTimeLabel(item.receivedAt)}</small>
                      </div>
                      <span>{item.body || item.appName}</span>
                    </button>
                  {:else}
                    <div class="notification-body">
                      <div class="notification-title-row">
                        <strong>{item.summary}</strong>
                        <small>{notificationTimeLabel(item.receivedAt)}</small>
                      </div>
                      <span>{item.body || item.appName}</span>
                    </div>
                  {/if}
                  {#if visibleNotificationActions(item).length > 0}
                    <div class="notification-actions">
                      {#each visibleNotificationActions(item) as action (action.key)}
                        <button
                          type="button"
                          class="notification-action"
                          onclick={(event) => {
                            event.stopPropagation();
                            notificationAction(item.id, action.key);
                          }}
                        >
                          {action.label}
                        </button>
                      {/each}
                    </div>
                  {/if}
                  <button
                    type="button"
                    class="icon-button"
                    aria-label="Close notification"
                    onclick={(event) => {
                      event.stopPropagation();
                      closeNotification(item.id);
                    }}
                  >
                    <Icon name="close" />
                  </button>
                </article>
              {/each}
            </div>
          </section>
        {/each}
      {/if}
    </div>
  </section>

  <aside class="calendar" aria-label="Calendar" onwheel={scrollMonth}>
    <header class="calendar-header">
      <span>{todayLabel}</span>
      <strong>{todayFull}</strong>
    </header>

    {#key monthLabel}
      <div class="calendar-month">
        <button type="button" class="calendar-nav" aria-label="Previous month" onclick={previousMonth}>
          <Icon name="chevron-left" />
        </button>
        <button type="button" class="calendar-current" onclick={resetMonth} onkeydown={calendarCurrentKeydown}>{monthLabel}</button>
        <button type="button" class="calendar-nav" aria-label="Next month" onclick={nextMonth}>
          <Icon name="chevron-right" />
        </button>
      </div>

      <div class="calendar-grid">
        {#each ["S", "M", "T", "W", "T", "F", "S"] as day, index (`${day}-${index}`)}
          <span>{day}</span>
        {/each}
        {#each cells as cell (`${cell.date.toISOString()}-${cell.outside}`)}
          <button
            type="button"
            class="calendar-day"
            class:outside={cell.outside}
            class:today={sameDay(cell.date, today)}
            class:is-selected={calendarKey(cell.date) === activeDay}
            aria-label={calendarLabel(cell.date)}
            aria-pressed={calendarKey(cell.date) === activeDay}
            onclick={() => selectDay(cell.date)}
          >
            {cell.date.getDate()}
          </button>
        {/each}
      </div>
    {/key}
  </aside>

  {#if snapshot.debugOverlay}
    <DebugMeter surface="DATE" />
  {/if}
</section>
