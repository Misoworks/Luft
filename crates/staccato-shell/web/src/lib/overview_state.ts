import type { ApplicationItem, ShellSnapshot } from "../shell/model";

export function filteredApplications(snapshot: ShellSnapshot, query: string) {
  const needle = query.trim().toLowerCase();
  if (!needle) return snapshot.applications;
  return snapshot.applications.filter((app) =>
    [app.name, app.comment ?? "", app.command].some((value) => value.toLowerCase().includes(needle)),
  );
}

export function selectedApplication(apps: ApplicationItem[], selection: number) {
  return apps[Math.max(0, Math.min(selection, apps.length - 1))];
}
