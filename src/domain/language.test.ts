import { describe, expect, it } from "vitest";
import { hasTranslatableText, isMostlyJapanese } from "./language";
import type { SourceDocument } from "./types";

const doc = (markdown: string): SourceDocument => ({ markdown, elements: [] });

describe("isMostlyJapanese", () => {
  it.each([
    ["Japanese prose", "これは日本語の文章です。翻訳は不要です。", true],
    ["Japanese with English terms", "ReactのコンポーネントはpropsでUIを返す関数です。", true],
    ["Japanese around a token", "次のコード ⟦K0⟧ を実行してください。", true],
    ["Kanji-heavy technical Japanese", "本論文では、大規模言語模型の推論効率向上手法を提案し、実験により有効性を検証した。", true],
    ["Kanji-heavy legal Japanese", "第三条 甲は乙に対し、本契約締結日から起算して三十日以内に代金全額を支払う。", true],
    ["Kanji-heavy Japanese headline", "東京都知事選挙の投票率は過去最低を更新した。", true],
    ["Katakana-heavy Japanese title", "プログラミング言語入門", true],
    ["Katakana-only Japanese title", "ダウンロード・センター", true],
    // Kanji with almost no kana reads as Chinese too, so it goes to Claude rather than risk a false skip.
    ["Kanji-only Japanese headline", "首相官邸で記者会見", false],
    ["Simplified Chinese", "这是一个中文句子，需要翻译成日语。", false],
    [
      "Chinese quoting a Japanese name in katakana",
      "这是一篇关于ポケモン新游戏的中文报道，玩家们非常期待这次的更新内容和新角色。",
      false,
    ],
    ["Short Chinese around a katakana name", "这是ポケモン的新游戏。", false],
    ["Chinese using の as slang", "我の朋友今天去北京参加了一个非常重要的会议，讨论了很多问题。", false],
    ["Chinese with a katakana middle dot", "迈克尔・乔丹是历史上最伟大的篮球运动员之一，他带领公牛队六次夺得总冠军。", false],
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
