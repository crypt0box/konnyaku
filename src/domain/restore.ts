import { PARTIAL_TOKEN_AT_END, TOKEN_PATTERN } from "./token";
import type { NonTranslatedElement, SourceDocument } from "./types";

export function restore(translatedMarkdown: string, doc: SourceDocument): string {
  const complete = translatedMarkdown.replace(PARTIAL_TOKEN_AT_END, "");
  return complete
    .split("\n")
    .map((line) => restoreLine(line, doc.elements))
    .join("\n");
}

function restoreLine(line: string, elements: NonTranslatedElement[]): string {
  const alone = /^(\s*(?:[-*+]\s+|\d+[.)]\s+|>\s*)*)⟦K(\d+)⟧\s*$/.exec(line);
  const element = alone ? elements[Number(alone[2])] : undefined;
  if (alone && element?.kind === "codeBlock") {
    const indent = alone[1].replace(/[^\s]/g, " ");
    const fenced = fence(element.lang, element.code).split("\n");
    return [alone[1] + fenced[0], ...fenced.slice(1).map((l) => indent + l)].join("\n");
  }
  return line.replace(TOKEN_PATTERN, (_, id: string) => inline(elements[Number(id)]));
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
