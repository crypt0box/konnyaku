export type Capture = {
  html?: string;
  text?: string;
  /** data: URI */
  imagePng?: string;
  source: "selection" | "clipboard";
};

export type TranslateError =
  | { kind: "claudeNotFound" }
  | { kind: "notLoggedIn" }
  | { kind: "rateLimited"; resetsAt?: string }
  | { kind: "processFailed"; detail: string };

export type TranslateEvent =
  | { kind: "delta"; text: string }
  | { kind: "done"; truncated: boolean }
  | { kind: "error"; error: TranslateError };

export type NonTranslatedElement =
  | { kind: "image"; src: string; alt: string }
  | { kind: "codeBlock"; lang: string; code: string }
  | { kind: "inlineCode"; code: string };

export type SourceDocument = {
  /** Markdown in which every 非翻訳要素 is replaced by its token. */
  markdown: string;
  /** Index is the token id. */
  elements: NonTranslatedElement[];
};

export type Style = "followSource" | "desuMasu" | "daDearu";
export type Model = "haiku" | "sonnet" | "opus";
export type Effort = "low" | "medium" | "high" | "xhigh" | "max";

export type Settings = {
  shortcut: string;
  model: Model;
  effort: Effort;
  style: Style;
  claudePath: string | null;
  launchAtLogin: boolean;
};
