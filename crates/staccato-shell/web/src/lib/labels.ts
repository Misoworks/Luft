import type { ShellSnapshot } from "../shell/model";

export function networkLabel(snapshot: ShellSnapshot) {
  const network = snapshot.status.network;
  if (!network) return "Offline";
  return network.wireless ? network.name : `Wired ${network.name}`;
}

export function audioLabel(snapshot: ShellSnapshot) {
  const audio = snapshot.status.audio;
  if (!audio) return "Unavailable";
  return audio.muted ? `Muted ${audio.percent}%` : `${audio.percent}%`;
}

export function batteryLabel(snapshot: ShellSnapshot) {
  const battery = snapshot.status.battery;
  if (!battery) return "No battery";
  return `${battery.percent}% ${battery.state}`;
}

export function nextProfileLabel(snapshot: ShellSnapshot) {
  const profiles = snapshot.profiles;
  if (profiles.length <= 1) return snapshot.activeProfile;
  const current = Math.max(0, profiles.findIndex((profile) => profile.active));
  const next = profiles[(current + 1 + profiles.length) % profiles.length]!;
  return `${snapshot.activeProfile} -> ${next.name}`;
}

export function shortDate() {
  return new Date().toLocaleDateString([], { month: "short", day: "numeric" });
}
