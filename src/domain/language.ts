import { TOKEN_PATTERN } from "./token";
import type { SourceDocument } from "./types";

/** Chinese borrows katakana for Japanese names, so katakana alone never proves Japanese. */
const HIRAGANA = /[ぁ-ゖゝ-ゟ]/u;
/** Excludes ・ and ー, which Chinese also uses. */
const KANA = /[ぁ-ゖゝ-ゟァ-ヺヽ-ヿｦ-ｯｱ-ﾝ]/gu;
const HAN = /[㐀-䶿一-鿿豈-﫿]/gu;
const JAPANESE_CHAR = /[぀-ゟ゠-ヿｦ-ﾟ㐀-䶿一-鿿豈-﫿]/gu;
const OTHER_WORD = /[^\P{L}぀-ゟ゠-ヿｦ-ﾟ㐀-䶿一-鿿豈-﫿]+/gu;

/**
 * Kana share of kana + Han. Kanji-dense Japanese (legal, academic, headlines) measures 0.24 to 0.29, while Chinese
 * that quotes a Japanese name or uses の as slang stays near 0.1.
 */
const MIN_KANA_SHARE = 0.2;
/** Share of Japanese in the text, counted so one Japanese character weighs the same as one word in another script. */
const MOSTLY_JAPANESE_SHARE = 0.8;

function prose(doc: SourceDocument): string {
  return doc.markdown
    .replace(TOKEN_PATTERN, " ")
    .replace(/\]\([^)]*\)/g, "]")
    .replace(/\b(?:https?|ftp|mailto):[^\s)>\]]+/gi, " ");
}

const count = (text: string, pattern: RegExp) => text.match(pattern)?.length ?? 0;

export function isMostlyJapanese(doc: SourceDocument): boolean {
  const text = prose(doc);
  if (!HIRAGANA.test(text)) return false;
  const kana = count(text, KANA);
  if (kana / (kana + count(text, HAN)) < MIN_KANA_SHARE) return false;
  const japanese = count(text, JAPANESE_CHAR);
  return japanese / (japanese + count(text, OTHER_WORD)) >= MOSTLY_JAPANESE_SHARE;
}

export function hasTranslatableText(doc: SourceDocument): boolean {
  return /\p{L}/u.test(prose(doc));
}
