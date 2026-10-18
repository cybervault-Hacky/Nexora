export interface ScrollMetrics {
  scrollTop: number;
  scrollHeight: number;
  clientHeight: number;
}

/** Distance from the bottom, in pixels, that still counts as "following". */
export const SCROLL_BOTTOM_THRESHOLD = 96;

/**
 * True when the view is at (or close to) the bottom, or when there is nothing
 * to scroll. New messages only pull the view down in that case; a user reading
 * earlier output is never yanked away.
 */
export function isNearBottom(
  metrics: ScrollMetrics,
  threshold: number = SCROLL_BOTTOM_THRESHOLD,
): boolean {
  if (metrics.scrollHeight <= metrics.clientHeight) {
    return true;
  }

  const distance = metrics.scrollHeight - metrics.scrollTop - metrics.clientHeight;
  return distance <= threshold;
}
