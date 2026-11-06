export type Step =
  | "guide"
  | "server"
  | "protocol"
  | "payment"
  | "install"
  | "ready";
export const steps: Step[] = [
  "guide",
  "server",
  "protocol",
  "payment",
  "install",
  "ready",
];
export function escapeHtml(value: string): string {
  return value.replace(
    /[&<>"']/g,
    (c) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[
        c
      ]!,
  );
}
export function validIp(value: string): boolean {
  return (
    /^\d{1,3}(\.\d{1,3}){3}$/.test(value) &&
    value
      .split(".")
      .every((p) => Number(p) <= 255 && String(Number(p)) === p) &&
    value !== "0.0.0.0"
  );
}
export function canDeploy(
  session: string,
  license: string,
  accepted: boolean,
  freePreview = false,
): boolean {
  return Boolean(session && (freePreview || license) && accepted);
}
