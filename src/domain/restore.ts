import { PARTIAL_TOKEN_AT_END, TOKEN_PATTERN } from "./token";
import type { NonTranslatedElement, SourceDocument } from "./types";

export function restore(translatedMarkdown: string, doc: SourceDocument): string {
  const complete = withoutModelImages(translatedMarkdown.replace(PARTIAL_TOKEN_AT_END, ""));
  return complete
    .split("\n")
    .map((line) => restoreLine(line, doc.elements))
    .join("\n");
}

/**
 * Every image of the 原文 comes back through a token, so an image the model wrote would only load a URL the 原文
 * never had: a tracker, or an exfiltration channel for prompt-injected text. Inline images keep their alt text.
 * Every other `![` gets its `!` escaped, which turns reference images into plain links and leaves prose such as
 * 注意![重要] as written.
 */
function withoutModelImages(markdown: string): string {
  return markdown
    .replace(/!\[((?:\\.|[^\\\]])*)\]\((?:\\.|[^\\)])*\)/g, "$1")
    .replace(/(\\*)!\[/g, (match, slashes: string) => (slashes.length % 2 ? match : `${slashes}\\![`));
}

function restoreLine(line: string, elements: NonTranslatedElement[]): string {
  const alone = /^(\s*(?:[-*+]\s+|\d+[.)]\s+|>\s*)*)⟦K(\d+)⟧\s*$/.exec(line);
  const element = alone ? elements[Number(alone[2])] : undefined;
  if (alone && element?.kind === "codeBlock") {
    // A blockquote ends at the first line without its `>`, so continuation lines repeat it; list markers become indentation.
    const indent = alone[1].replace(/[-*+]|\d+[.)]/g, (marker) => " ".repeat(marker.length));
    const fenced = fence(element.lang, element.code).split("\n");
    return [alone[1] + fenced[0], ...fenced.slice(1).map((l) => indent + l)].join("\n");
  }
  // A table row splits its cells at every unescaped pipe, even inside inline code, alt text, and URLs.
  const inTableRow = /^\s*(?:>\s*)*\|/.test(line);
  return line.replace(TOKEN_PATTERN, (_, id: string) => {
    const markdown = inline(elements[Number(id)]);
    return inTableRow ? markdown.replace(/\|/g, "\\|") : markdown;
  });
}

function inline(element: NonTranslatedElement | undefined): string {
  switch (element?.kind) {
    case undefined:
      return "";
    case "image":
      return `![${escapeAlt(element.alt)}](<${element.src.replace(/[<>\s]/g, encodeURIComponent)}>)`;
    case "inlineCode":
      return inlineCode(element.code);
    case "codeBlock":
      return inlineCode(element.code.replace(/\n/g, " "));
  }
}

function longestBacktickRun(text: string): number {
  return Math.max(0, ...(text.match(/`+/g) ?? []).map((run) => run.length));
}

function inlineCode(code: string): string {
  const ticks = "`".repeat(longestBacktickRun(code) + 1);
  const pad = code.startsWith("`") || code.endsWith("`") ? " " : "";
  return `${ticks}${pad}${code}${pad}${ticks}`;
}

function fence(lang: string, code: string): string {
  const ticks = "`".repeat(Math.max(3, longestBacktickRun(code) + 1));
  return `${ticks}${lang}\n${code}\n${ticks}`;
}

function escapeAlt(alt: string): string {
  return alt.replace(/[\\[\]]/g, "\\$&").replace(/\s+/g, " ");
}
