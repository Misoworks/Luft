<script lang="ts">
  import DebugMeter from "./DebugMeter.svelte";
  import Icon from "./Icon.svelte";
  import { sendAction } from "../shell/bridge";
  import { calendarCells, sameDay } from "../lib/calendar";
  import type { ShellSnapshot } from "../shell/model";

  let { snapshot }: { snapshot: ShellSnapshot } = $props();
  let calendarMonthOffset = $state(0);

  const today = $derived(new Date());
  const month = $derived(new Date(today.getFullYear(), today.getMonth() + calendarMonthOffset, 1));
  const cells = $derived(calendarCells(month));

  function scrollMonth(event: WheelEvent) {
    event.preventDefault();
    calendarMonthOffset += event.deltaY > 0 ? 1 : -1;
  }

  function closeNotification(notification: number) {
    sendAction({ type: "notification-close", notification });
  }

  function notificationAction(notification: number, action: string) {
    sendAction({ type: "notification-action", notification, action });
  }
</script>

<section class="popover date-center" onwheel={scrollMonth}>
  <div class="notifications">
    {#if snapshot.notifications.length === 0}
      <div class="notification-empty">
        <Icon name="bell" />
        <span>No Notifications</span>
      </div>
    {:else}
      {#each snapshot.notifications as item (item.id)}
        <article class="notification {item.urgency}">
          <div class="notification-body">
            <strong>{item.summary}</strong>
            <span>{item.body || item.appName}</span>
            {#if item.actions.length > 0}
              <div class="notification-actions">
                {#each item.actions.slice(0, 3) as action (action.key)}
                  <button type="button" class="notification-action" onclick={() => notificationAction(item.id, action.key)}>
                    {action.label}
                  </button>
                {/each}
              </div>
            {/if}
          </div>
          <button type="button" class="icon-button" aria-label="Close notification" onclick={() => closeNotification(item.id)}>
            <Icon name="close" />
          </button>
        </article>
      {/each}
    {/if}
  </div>

  <aside class="calendar">
    <header class="calendar-header">
      <span>{today.toLocaleDateString([], { weekday: "long" })}</span>
      <strong>{today.toLocaleDateString([], { month: "long", day: "numeric", year: "numeric" })}</strong>
    </header>

    <div class="calendar-month">
      <strong>{month.toLocaleDateString([], { month: "long", year: "numeric" })}</strong>
    </div>

    <div class="calendar-grid">
      {#each ["S", "M", "T", "W", "T", "F", "S"] as day, index (`${day}-${index}`)}
        <span>{day}</span>
      {/each}
      {#each cells as cell (`${cell.date.toISOString()}-${cell.outside}`)}
        <span class:outside={cell.outside} class:today={sameDay(cell.date, today)}>{cell.date.getDate()}</span>
      {/each}
    </div>
  </aside>

  {#if snapshot.debugOverlay}
    <DebugMeter surface="DATE" />
  {/if}
</section>
