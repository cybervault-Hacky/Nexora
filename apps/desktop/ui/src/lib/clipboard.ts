/**
 * Copies text with the async Clipboard API, falling back to the legacy
 * selection trick for a webview that does not expose it.
 *
 * Returns a boolean and never throws: a caller that cannot copy must be able to
 * say so instead of claiming success.
 */
export async function copyText(text: string): Promise<boolean> {
  if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text);

      return true;
    } catch {
      // Permission denied or an insecure context: try the fallback below.
    }
  }

  if (typeof document === "undefined") {
    return false;
  }

  const active = document.activeElement as HTMLElement | null;
  const area = document.createElement("textarea");

  area.value = text;
  area.setAttribute("readonly", "");
  area.setAttribute("aria-hidden", "true");
  area.style.position = "fixed";
  area.style.top = "-1000px";
  area.style.opacity = "0";
  document.body.appendChild(area);

  let copied = false;

  try {
    area.select();
    copied = document.execCommand?.("copy") === true;
  } catch {
    copied = false;
  } finally {
    area.remove();
    active?.focus?.();
  }

  return copied;
}
