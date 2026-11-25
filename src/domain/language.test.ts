import { describe, expect, it } from "vitest";
import { hasTranslatableText, isMostlyJapanese } from "./language";
import type { SourceDocument } from "./types";

const doc = (markdown: string): SourceDocument => ({ markdown, elements: [] });

describe("isMostlyJapanese", () => {
  it.each([
    ["Japanese prose", "これは日本語の文章です。翻訳は不要です。", true],
    ["Japanese with English terms", "ReactのコンポーネントはpropsでUIを返す関数です。", true],
    ["Japanese around a token", "次のコード ⟦K0⟧ を実行してください。", true],
    ["Simplified Chinese", "这是一个中文句子，需要翻译成日语。", false],
    ["Traditional Chinese", "這是一個中文句子，需要翻譯成日語。", false],
    ["English", "This is an English sentence.", false],
    ["Korean", "이것은 한국어 문장입니다.", false],
    [
      "English paragraphs with one Japanese line",
      "The quick brown fox jumps over the lazy dog near the river bank today.\n\nこれは日本語です。",
      false,
    ],
  ])("%s → %s", (_, markdown, expected) => {
    expect(isMostlyJapanese(doc(markdown))).toBe(expected);
  });
});

describe("hasTranslatableText", () => {
  it("is false for tokens, numbers, and punctuation only", () => {
    expect(hasTranslatableText(doc("⟦K0⟧\n\n⟦K1⟧ 123 — !"))).toBe(false);
  });

  it("is true once a word appears", () => {
    expect(hasTranslatableText(doc("⟦K0⟧ Figure 1"))).toBe(true);
  });
});
