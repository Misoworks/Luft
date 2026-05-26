import type { DockApp } from "../shell/model";

export function appIconSignature(app: DockApp) {
  return app.iconUri ?? "app";
}
