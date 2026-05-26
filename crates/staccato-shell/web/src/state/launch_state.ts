const launchingCommands = new Map<string, number>();
const launchDuration = 520;

export function isLaunching(command: string) {
  expireLaunches();
  return launchingCommands.has(command);
}

export function markLaunching(command: string, done: () => void) {
  const launchUntil = Date.now() + launchDuration;
  launchingCommands.set(command, launchUntil);
  window.setTimeout(() => {
    if (launchingCommands.get(command) !== launchUntil) return;
    launchingCommands.delete(command);
    done();
  }, launchDuration);
}

export function expireLaunches() {
  const now = Date.now();
  launchingCommands.forEach((until, command) => {
    if (until <= now) {
      launchingCommands.delete(command);
    }
  });
}
