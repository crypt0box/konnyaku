import MarkdownIt from "markdown-it";
import TurndownService from "turndown";
import { gfm } from "turndown-plugin-gfm";
import { token } from "./token";
import type { Capture, NonTranslatedElement, SourceDocument } from "./types";

const plainTextAsMarkdown = new MarkdownIt({ html: false, breaks: true, linkify: false });

const DROPPED_TAGS = "script, style, noscript, template, head, meta, link, iframe, object, embed, svg, canvas";

export function toSourceDocument(capture: Capture): SourceDocument | null {
  const html = sourceHtml(capture);
  if (html === null) return null;

  const body = new DOMParser().parseFromString(html, "text/html").body;
  body.querySelectorAll(DROPPED_TAGS).forEach((el) => el.remove());

  const elements: NonTranslatedElement[] = [];
  const replaceWithToken = (el: Element, element: NonTranslatedElement, block: boolean) => {
    const id = elements.push(element) - 1;
    const doc = el.ownerDocument;
    if (block) {
      const p = doc.createElement("p");
      p.textContent = token(id);
      el.replaceWith(p);
    } else {
      el.replaceWith(doc.createTextNode(token(id)));
    }
  };

  for (const el of Array.from(body.querySelectorAll("pre, img, code"))) {
    if (!el.isConnected) continue;
    if (el.tagName === "PRE") {
      replaceWithToken(el, { kind: "codeBlock", lang: codeLanguage(el), code: preText(el) }, true);
    } else if (el.tagName === "IMG") {
      const src = el.getAttribute("src")?.trim();
      if (src) replaceWithToken(el, { kind: "image", src, alt: el.getAttribute("alt")?.trim() ?? "" }, false);
      else el.remove();
    } else {
      replaceWithToken(el, { kind: "inlineCode", code: el.textContent ?? "" }, false);
    }
  }

  const markdown = turndown().turndown(body).trim();
  if (markdown === "" && elements.length === 0) return null;
  return { markdown, elements };
}

function sourceHtml(capture: Capture): string | null {
  if (capture.html && capture.html.trim() !== "") return capture.html;
  if (capture.text && capture.text.trim() !== "") return plainTextAsMarkdown.render(capture.text);
  if (capture.imagePng) return `<img src="${capture.imagePng}" alt="">`;
  return null;
}

function codeLanguage(pre: Element): string {
  const classes = [pre.className, pre.querySelector("code")?.className ?? ""].join(" ");
  return /(?:^|\s)(?:language|lang)-([\w+#.-]+)/.exec(classes)?.[1] ?? "";
}

function preText(pre: Element): string {
  const clone = pre.cloneNode(true) as Element;
  clone.querySelectorAll("br").forEach((br) => br.replaceWith("\n"));
  return (clone.textContent ?? "").replace(/\n$/, "");
}

function turndown(): TurndownService {
  const service = new TurndownService({
    headingStyle: "atx",
    codeBlockStyle: "fenced",
    bulletListMarker: "-",
    emDelimiter: "*",
  });
  service.use(gfm);
  return service;
}
