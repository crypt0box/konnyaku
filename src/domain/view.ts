import type { SourceDocument, TranslateError } from "./types";

export type View =
  | { kind: "empty" }
  | { kind: "translating"; doc: SourceDocument; partial: string }
  | { kind: "translated"; doc: SourceDocument; text: string; truncated: boolean }
  | { kind: "alreadyJapanese"; doc: SourceDocument }
  | { kind: "untranslatable"; doc: SourceDocument }
  | { kind: "failed"; error: TranslateError | { kind: "nothingCaptured" } };
