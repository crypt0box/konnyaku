import { useCallback, useEffect, useRef, useState } from "react";
import { hasTranslatableText, isMostlyJapanese } from "../../domain/language";
import { buildSystemPrompt, buildUserMessage } from "../../domain/prompt";
import { toSourceDocument } from "../../domain/sourceDocument";
import type { Capture, SourceDocument } from "../../domain/types";
import type { View } from "../../domain/view";
import { getSettings, onCancelled, onCaptured, translate } from "../../ipc";

/** Drives the 翻訳ウィンドウ's View from captures and translation engine events. */
export function useTranslation(): View {
  const [view, setView] = useState<View>({ kind: "empty" });
  const currentJob = useRef(0);

  const start = useCallback(async (capture: Capture) => {
    const job = ++currentJob.current;
    const doc = toSourceDocument(capture);
    if (doc === null) return setView({ kind: "failed", error: { kind: "nothingCaptured" } });
    if (!hasTranslatableText(doc)) return setView({ kind: "untranslatable", doc });
    if (isMostlyJapanese(doc)) return setView({ kind: "alreadyJapanese", doc });

    setView({ kind: "translating", doc, partial: "" });
    const isCurrent = () => job === currentJob.current;
    const { style } = await getSettings();
    // A newer capture may have started its job while this one awaited; starting now would kill it.
    if (!isCurrent()) return;
    let text = "";
    let frame: number | null = null;
    const showPartial = (doc: SourceDocument) => {
      frame = null;
      setView((v) => (v.kind === "translating" && v.doc === doc ? { ...v, partial: text } : v));
    };

    await translate(buildSystemPrompt(style), buildUserMessage(doc.markdown), (event) => {
      if (!isCurrent()) return;
      switch (event.kind) {
        case "delta":
          text += event.text;
          frame ??= requestAnimationFrame(() => showPartial(doc));
          return;
        case "done":
          if (frame !== null) cancelAnimationFrame(frame);
          return setView({ kind: "translated", doc, text, truncated: event.truncated });
        case "error":
          if (frame !== null) cancelAnimationFrame(frame);
          return setView({ kind: "failed", error: event.error });
      }
    });
  }, []);

  useEffect(() => {
    const unlisten = [
      onCaptured((capture) => void start(capture)),
      onCancelled(() => {
        currentJob.current++;
        setView((v) => (v.kind === "translating" ? { kind: "empty" } : v));
      }),
    ];
    return () => unlisten.forEach((p) => void p.then((stop) => stop()));
  }, [start]);

  return view;
}
