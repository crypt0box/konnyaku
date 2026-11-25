import { describe, expect, it } from "vitest";
import { renderMarkdown } from "./render";
import { restore } from "./restore";
import type { SourceDocument } from "./types";

const doc: SourceDocument = {
  markdown: "",
  elements: [
    { kind: "inlineCode", code: "a`b" },
    { kind: "codeBlock", lang: "sh", code: "echo ```\nexit 0" },
    { kind: "image", src: "https://example.com/a (1).png", alt: "a [b]" },
  ],
};

describe("restore", () => {
  it("never shows a half-streamed token", () => {
    const full = "実行 ⟦K0⟧ する。\n\n⟦K1⟧\n\n図 ⟦K2⟧ 終わり";
    for (let end = 1; end <= full.length; end++) {
      const html = renderMarkdown(restore(full.slice(0, end), doc));
      expect(html, full.slice(0, end)).not.toMatch(/⟦|⟧|K\d/);
    }
  });

  it("restores each element so Markdown renders it intact", () => {
    const container = document.createElement("div");
    container.innerHTML = renderMarkdown(restore("実行 ⟦K0⟧ する。\n\n⟦K1⟧\n\n図 ⟦K2⟧", doc));
    expect(container.querySelector("p code")?.textContent).toBe("a`b");
    expect(container.querySelector("pre code")?.textContent).toBe("echo ```\nexit 0\n");
    expect(container.querySelector("img")?.getAttribute("alt")).toBe("a [b]");
    expect(container.querySelector("img")?.getAttribute("src")).toBe("https://example.com/a%20(1).png");
  });

  it("keeps a code block inside the list item the model placed it in", () => {
    const container = document.createElement("div");
    container.innerHTML = renderMarkdown(restore("- 手順\n\n  ⟦K1⟧\n- 次", doc));
    expect(container.querySelector("li pre code")?.textContent).toBe("echo ```\nexit 0\n");
    expect(container.querySelectorAll("li")).toHaveLength(2);
  });

  it("drops tokens the document does not have", () => {
    expect(restore("あ⟦K9⟧い", doc)).toBe("あい");
  });
});

describe("renderMarkdown", () => {
  it("shows a box with the alt text for an image the window cannot load", () => {
    const container = document.createElement("div");
    container.innerHTML = renderMarkdown("![図1](<file:///Users/me/a.png>) と ![](<https://e.com/x.png>)");
    const box = container.querySelector(".unloadable-image");
    expect(box?.textContent).toBe("図1");
    expect(box?.querySelector("a")).toBeNull();
    expect(container.querySelector("img")?.getAttribute("src")).toBe("https://e.com/x.png");
  });

  it("never produces live script from untrusted output", () => {
    const container = document.createElement("div");
    container.innerHTML = renderMarkdown('<img src=x onerror="alert(1)">\n\n[x](javascript:alert(1))');
    expect(container.querySelector("[onerror]")).toBeNull();
    expect(container.querySelector("a[href^='javascript']")).toBeNull();
  });
});
