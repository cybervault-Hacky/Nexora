export type ComposerMode = "auto" | "image" | "video";
export type MediaTask =
  | "image_generation"
  | "image_editing"
  | "video_generation"
  | "video_editing";

const VIDEO_NOUN = "(?:video|clip|film|animation)";
const IMAGE_NOUN = "(?:image|picture|photo|photograph|illustration|drawing|artwork|logo)";
const CREATE_VERB = "(?:generate|create|make|render|produce|animate)";
const IMAGE_VERB = "(?:draw|paint|illustrate|generate|create|make|render)";
const EDIT_VERB = "(?:edit|change|modify|retouch|remove|replace|crop)";
const CODE_CONSTRUCTION =
  /\b(?:write|implement|code|develop|debug|refactor|fix)\b[^\n]{0,64}\b(?:function|class|script|program|app|application|website|component|api|library|algorithm|module|editor|player|generator|gallery)\b|\b(?:create|make|build)\s+(?:(?:me|us)\s+)?(?:(?:a|an|the)\s+)?(?:function|class|script|program|app|application|website|component|api|library|algorithm|module|editor|player|generator|gallery)\b/i;
const MEDIA_CODE_COMPOUND = /\b(?:video|image|photo)\s+(?:game|editor|player|gallery|generator|viewer|browser|processor|component)\b/i;
const MEDIA_TEXT_ARTIFACT =
  /\b(?:video|clip|film|image|picture|photo|animation)\s+(?:prompt|script|storyboard|outline|plan|concept|description|caption|alt text)\b|\b(?:prompt|script|storyboard|outline|plan|concept|description|caption|alt text)\b[^\n]{0,32}\b(?:video|clip|film|image|picture|photo|animation)\b/i;

function hasActionForNoun(text: string, verbs: string, nouns: string): boolean {
  const forward = new RegExp(`\\b${verbs}\\b[^\\n]{0,64}\\b${nouns}\\b`, "i");
  const reverse = new RegExp(`\\b${nouns}\\b[^\\n]{0,48}\\b${verbs}\\b`, "i");
  return forward.test(text) || reverse.test(text);
}

function isImageEditRequest(text: string): boolean {
  return hasActionForNoun(text, EDIT_VERB, "(?:image|picture|photo|photograph)");
}

function isVideoEditRequest(text: string): boolean {
  return hasActionForNoun(text, EDIT_VERB, "(?:video|clip|film)");
}

/**
 * Conservative, deterministic local intent routing. Ambiguous wording remains
 * an ordinary chat turn; no classifier model or remote service is used.
 */
export function resolveMediaTask(mode: ComposerMode, prompt: string): MediaTask | null {
  const text = prompt.trim();
  if (!text) return null;

  if (mode === "video") return isVideoEditRequest(text) ? "video_editing" : "video_generation";
  if (mode === "image") return isImageEditRequest(text) ? "image_editing" : "image_generation";
  // Keep software and text-about-media requests ordinary, not media inference intents.
  if (
    CODE_CONSTRUCTION.test(text) ||
    MEDIA_CODE_COMPOUND.test(text) ||
    MEDIA_TEXT_ARTIFACT.test(text)
  ) return null;
  if (isVideoEditRequest(text)) return "video_editing";
  if (hasActionForNoun(text, CREATE_VERB, VIDEO_NOUN)) return "video_generation";
  if (isImageEditRequest(text)) return "image_editing";
  if (hasActionForNoun(text, IMAGE_VERB, IMAGE_NOUN)) return "image_generation";
  return null;
}
