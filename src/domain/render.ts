import DOMPurify from "dompurify";
import MarkdownIt from "markdown-it";

const markdown = new MarkdownIt({ html: false, linkify: true, breaks: true });
const defaultValidateLink = markdown.validateLink.bind(markdown);
// file: images must parse as images so they can become an unloadable-image box instead of raw Markdown text.
markdown.validateLink = (url) => /^\s*file:/i.test(url) || defaultValidateLink(url);

const LOADABLE_IMAGE = /^(?:https?:|data:image\/)/i;
export const OPENABLE_LINK = /^(?:https?:|mailto:)/i;

/** Renders untrusted Markdown to sanitized HTML. Images the window cannot load become a box with the alt text. */
export function renderMarkdown(source: string): string {
  const body = DOMPurify.sanitize(markdown.render(source), { RETURN_DOM: true });
  for (const img of Array.from(body.querySelectorAll("img"))) {
    if (!LOADABLE_IMAGE.test(img.getAttribute("src") ?? "")) img.replaceWith(unloadableImage(img));
  }
  return body.innerHTML;
}

export function unloadableImage(img: HTMLImageElement): HTMLElement {
  const doc = img.ownerDocument;
  const box = doc.createElement("span");
  box.className = "unloadable-image";
  const alt = doc.createElement("span");
  alt.className = "unloadable-image-alt";
  alt.textContent = img.getAttribute("alt") || "画像";
  box.append(alt);
  const src = img.getAttribute("src") ?? "";
  if (OPENABLE_LINK.test(src)) {
    const link = doc.createElement("a");
    link.href = src;
    link.textContent = "ブラウザで開く";
    box.append(link);
  }
  return box;
}
