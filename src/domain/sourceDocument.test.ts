import { describe, expect, it } from "vitest";
import { hasTranslatableText, isMostlyJapanese } from "./language";
import { renderMarkdown } from "./render";
import { restore } from "./restore";
import { toSourceDocument } from "./sourceDocument";

const RICH_HTML = `<html><head><style>h1 { color: red }</style><script>alert(1)</script></head><body>
<h1>Getting started</h1>
<p>Install the <code>konnyaku</code> package and read the <a href="https://example.com/docs">guide</a>.</p>
<ul><li>Fast setup</li><li>Small footprint</li></ul>
<table><thead><tr><th>Name</th><th>Value</th></tr></thead>
<tbody><tr><td>Speed</td><td>High</td></tr><tr><td>Cost</td><td>Low</td></tr></tbody></table>
<p><img src="https://example.com/diagram.png" alt="Architecture diagram"></p>
<pre><code class="language-ts">const greeting = "Hello, world";
console.log(\`\${greeting}\`);</code></pre>
</body></html>`;

const FAKE_DICTIONARY: Record<string, string> = {
  "Getting started": "はじめに",
  "Install the": "インストールする",
  "package and read the": "パッケージを読む",
  guide: "ガイド",
  "Fast setup": "高速なセットアップ",
  "Small footprint": "小さなフットプリント",
  Name: "名前",
  Value: "値",
  Speed: "速度",
  High: "高い",
  Cost: "コスト",
  Low: "低い",
};

function fakeTranslate(markdown: string): string {
  return Object.entries(FAKE_DICTIONARY).reduce((text, [en, ja]) => text.split(en).join(ja), markdown);
}

function renderToDom(markdown: string): HTMLElement {
  const container = document.createElement("div");
  container.innerHTML = renderMarkdown(markdown);
  return container;
}

describe("toSourceDocument → translation → restore", () => {
  const doc = toSourceDocument({ html: RICH_HTML, source: "clipboard" })!;

  it("never sends images or code to the translation engine", () => {
    expect(doc.markdown).not.toContain("diagram.png");
    expect(doc.markdown).not.toContain("console.log");
    expect(doc.markdown).not.toContain("konnyaku");
    expect(doc.markdown).not.toContain("alert");
    expect(doc.markdown).not.toContain("color: red");
  });

  it("keeps table, list, heading, image, and code in the rendered 訳文", () => {
    const view = renderToDom(restore(fakeTranslate(doc.markdown), doc));

    expect(view.querySelector("h1")?.textContent).toBe("はじめに");
    expect(Array.from(view.querySelectorAll("li"), (li) => li.textContent)).toEqual([
      "高速なセットアップ",
      "小さなフットプリント",
    ]);
    const rows = Array.from(view.querySelectorAll("table tr"), (tr) =>
      Array.from(tr.querySelectorAll("th, td"), (cell) => cell.textContent),
    );
    expect(rows).toEqual([
      ["名前", "値"],
      ["速度", "高い"],
      ["コスト", "低い"],
    ]);

    const img = view.querySelector("img");
    expect(img?.getAttribute("src")).toBe("https://example.com/diagram.png");
    expect(img?.getAttribute("alt")).toBe("Architecture diagram");

    const code = view.querySelector("pre code");
    expect(code?.className).toBe("language-ts");
    expect(code?.textContent).toBe('const greeting = "Hello, world";\nconsole.log(`${greeting}`);\n');

    expect(view.querySelector("p code")?.textContent).toBe("konnyaku");
    expect(view.querySelector("a[href='https://example.com/docs']")?.textContent).toBe("ガイド");
    expect(view.textContent).not.toMatch(/⟦|⟧/);
  });
});

describe("toSourceDocument", () => {
  it("returns null when nothing usable was captured", () => {
    expect(toSourceDocument({ source: "clipboard" })).toBeNull();
    expect(toSourceDocument({ text: "  \n ", source: "selection" })).toBeNull();
    expect(toSourceDocument({ html: "<script>alert(1)</script>", source: "selection" })).toBeNull();
  });

  it("keeps hard line breaks of plain text", () => {
    const doc = toSourceDocument({ text: "Line one\nLine two\n\nNext paragraph", source: "selection" })!;
    const view = renderToDom(restore(doc.markdown, doc));
    expect(view.querySelectorAll("br")).toHaveLength(1);
    expect(view.querySelectorAll("p")).toHaveLength(2);
  });

  it("treats plain text as Markdown", () => {
    const doc = toSourceDocument({ text: "# Title\n\n- `npm install`\n", source: "selection" })!;
    expect(doc.elements).toEqual([{ kind: "inlineCode", code: "npm install" }]);
    const view = renderToDom(restore(doc.markdown, doc));
    expect(view.querySelector("h1")?.textContent).toBe("Title");
    expect(view.querySelector("li code")?.textContent).toBe("npm install");
  });

  it("renders an image-only capture without anything to translate", () => {
    const png = "data:image/png;base64,iVBORw0KGgo=";
    const doc = toSourceDocument({ imagePng: png, source: "selection" })!;
    expect(hasTranslatableText(doc)).toBe(false);
    expect(renderToDom(restore(doc.markdown, doc)).querySelector("img")?.getAttribute("src")).toBe(png);
  });

  it("prefers HTML over the plain-text flavor of the same selection", () => {
    const doc = toSourceDocument({ html: "<h2>Hi there</h2>", text: "Hi there", source: "selection" })!;
    expect(doc.markdown).toBe("## Hi there");
  });

  it("does not count tokens or URLs as Japanese evidence", () => {
    const doc = toSourceDocument({ html: '<p><a href="https://例え.jp/ひらがな">Read more</a></p>', source: "selection" })!;
    expect(isMostlyJapanese(doc)).toBe(false);
  });

  it("keeps a table without a header row, as TextEdit and other RTF sources copy it", () => {
    // Shape of NSAttributedString's HTML export of a TextEdit table: no th, paragraphs inside cells.
    const cell = (text: string) => `<td valign="middle" class="td1"><p class="p3"><span class="s1">${text}</span></p></td>`;
    const html = `<table cellspacing="0" cellpadding="0" class="t1"><tbody>
<tr>${cell("<b>Part</b>")}${cell("<b>Interval</b>")}</tr>
<tr>${cell("Gasket")}${cell("Every year")}</tr>
<tr>${cell("Shower screen")}<td class="td1"><p class="p3">Every six</p><p class="p3">months<br>or sooner</p></td></tr>
</tbody></table>`;
    const doc = toSourceDocument({ html, source: "selection" })!;
    const view = renderToDom(restore(doc.markdown, doc));

    expect(view.textContent).not.toContain("<");
    const rows = Array.from(view.querySelectorAll("table tr"), (tr) =>
      Array.from(tr.querySelectorAll("th, td"), (c) => c.textContent),
    );
    expect(rows).toEqual([
      ["Part", "Interval"],
      ["Gasket", "Every year"],
      ["Shower screen", "Every six months or sooner"],
    ]);
  });
});
