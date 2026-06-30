import type { PanelApp } from "../shell/model";

export function appIconSignature(app: PanelApp) {
  return app.iconUri ?? "app";
}
