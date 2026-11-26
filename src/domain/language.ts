import { TOKEN_PATTERN } from "./token";
import type { SourceDocument } from "./types";

const HIRAGANA = /[ぁ-ゖゝ-ゟ]/gu;
/** Excludes ・ and ー, which Chinese also uses. */
const KATAKANA = /[ァ-ヺヽ-ヿｦ-ｯｱ-ﾝ]/gu;
const HAN = /[㐀-䶿一-鿿豈-﫿]/gu;
const JAPANESE_CHAR = /[぀-ゟ゠-ヿｦ-ﾟ㐀-䶿一-鿿豈-﫿]/gu;
const OTHER_WORD = /[^\P{L}぀-ゟ゠-ヿｦ-ﾟ㐀-䶿一-鿿豈-﫿]+/gu;

/**
 * Shares of kana + Han. Kanji-dense Japanese (legal, academic, headlines) has a hiragana share of 0.24 to 0.29,
 * while Chinese that uses の as slang stays near 0.03. Chinese borrows katakana only for names, so katakana proves
 * Japanese only when it outweighs the Han, as in a title such as プログラミング言語入門.
 */
const MIN_HIRAGANA_SHARE = 0.2;
const MIN_KATAKANA_SHARE = 0.5;
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
  const hiragana = count(text, HIRAGANA);
  const katakana = count(text, KATAKANA);
  const kanaAndHan = hiragana + katakana + count(text, HAN);
  if (hiragana < MIN_HIRAGANA_SHARE * kanaAndHan && katakana < MIN_KATAKANA_SHARE * kanaAndHan) return false;
  const japanese = count(text, JAPANESE_CHAR);
  return japanese / (japanese + count(text, OTHER_WORD)) >= MOSTLY_JAPANESE_SHARE;
}

export function hasTranslatableText(doc: SourceDocument): boolean {
  return /\p{L}/u.test(prose(doc));
}
