import type { ShellSnapshot } from "../shell/model";

export function networkLabel(snapshot: ShellSnapshot) {
  const network = snapshot.status.network;
  if (!network) return "Offline";
  return network.wireless ? network.name : `Wired ${network.name}`;
}

export function batteryLabel(snapshot: ShellSnapshot) {
  const battery = snapshot.status.battery;
  if (!battery) return "No battery";
  return `${battery.percent}% ${battery.state}`;
}

export function brightnessLabel(snapshot: ShellSnapshot) {
  const brightness = snapshot.status.brightness;
  if (!brightness) return "Unavailable";
  return `${brightness.percent}%`;
}

export function shortDate() {
  return new Date().toLocaleDateString([], { month: "short", day: "numeric" });
}
