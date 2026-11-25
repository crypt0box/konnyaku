import { TOKEN_PATTERN } from "./token";
import type { SourceDocument } from "./types";

const KANA = /[぀-ゟ゠-ヿｦ-ﾟ]/u;
const JAPANESE_CHAR = /[぀-ゟ゠-ヿｦ-ﾟ㐀-䶿一-鿿豈-﫿]/gu;
const OTHER_WORD = /[^\P{L}぀-ゟ゠-ヿｦ-ﾟ㐀-䶿一-鿿豈-﫿]+/gu;

/** Share of Japanese in the text, counted so one Japanese character weighs the same as one word in another script. */
const MOSTLY_JAPANESE_SHARE = 0.8;

function prose(doc: SourceDocument): string {
  return doc.markdown
    .replace(TOKEN_PATTERN, " ")
    .replace(/\]\([^)]*\)/g, "]")
    .replace(/\b(?:https?|ftp|mailto):[^\s)>\]]+/gi, " ");
}

export function isMostlyJapanese(doc: SourceDocument): boolean {
  const text = prose(doc);
  if (!KANA.test(text)) return false;
  const japanese = text.match(JAPANESE_CHAR)?.length ?? 0;
  const otherWords = text.match(OTHER_WORD)?.length ?? 0;
  return japanese / (japanese + otherWords) >= MOSTLY_JAPANESE_SHARE;
}

export function hasTranslatableText(doc: SourceDocument): boolean {
  return /\p{L}/u.test(prose(doc));
}
