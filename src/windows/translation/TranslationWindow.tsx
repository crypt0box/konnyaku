import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useMemo, useRef, useState, type MouseEvent, type ReactNode } from "react";
import { OPENABLE_LINK, renderMarkdown, unloadableImage } from "../../domain/render";
import { restore } from "../../domain/restore";
import type { Settings, SourceDocument, TranslateError } from "../../domain/types";
import type { View } from "../../domain/view";
import { debugDumpRendered, dismiss, getSettings, onSettingsChanged } from "../../ipc";
import { useTranslation } from "./useTranslation";

export function TranslationWindow() {
  const view = useTranslation();
  const shortcut = useShortcut();
  const contentRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") void dismiss();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    if (view.kind !== "empty" && view.kind !== "translating" && contentRef.current) {
      void debugDumpRendered(contentRef.current.outerHTML);
    }
  }, [view]);

  const shown = "doc" in view ? view.doc : view.kind;
  useEffect(() => {
    contentRef.current?.parentElement?.scrollTo({ top: 0 });
  }, [shown]);

  return (
    <main className="translation">
      <div id="content" ref={contentRef}>
        <ViewBody view={view} shortcut={shortcut} />
      </div>
    </main>
  );
}

function ViewBody({ view, shortcut }: { view: View; shortcut: string }) {
  switch (view.kind) {
    case "empty":
      return (
        <Notice title="翻訳したい部分を選択して、ショートカットを押してください">
          <p>
            <kbd>{shortcut}</kbd> で、選択中の内容を日本語で表示します。
          </p>
        </Notice>
      );
    case "translating":
      return view.partial === "" ? (
        <p className="status">翻訳しています…</p>
      ) : (
        <Rendered markdown={view.partial} doc={view.doc} />
      );
    case "translated":
      return (
        <>
          <Rendered markdown={view.text} doc={view.doc} />
          {view.truncated && <p className="truncated">訳文はここで途切れています。出力の上限に達しました。</p>}
        </>
      );
    case "untranslatable":
      return <Rendered markdown={view.doc.markdown} doc={view.doc} />;
    case "alreadyJapanese":
      return <Notice title="原文はすでに日本語です" />;
    case "failed":
      return <Failure error={view.error} shortcut={shortcut} />;
  }
}

function Rendered({ markdown, doc }: { markdown: string; doc: SourceDocument }) {
  const html = useMemo(() => renderMarkdown(restore(markdown, doc)), [markdown, doc]);
  const ref = useRef<HTMLElement>(null);

  useEffect(() => {
    for (const img of Array.from(ref.current?.querySelectorAll("img") ?? [])) {
      img.addEventListener("error", () => img.replaceWith(unloadableImage(img)), { once: true });
    }
  }, [html]);

  const openLink = (event: MouseEvent) => {
    const link = (event.target as Element).closest("a");
    if (!link) return;
    event.preventDefault();
    const href = link.getAttribute("href") ?? "";
    if (OPENABLE_LINK.test(href)) void openUrl(href);
  };

  return <article ref={ref} className="markdown" onClick={openLink} dangerouslySetInnerHTML={{ __html: html }} />;
}

function Notice({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <section className="notice">
      <h2>{title}</h2>
      {children}
    </section>
  );
}

function Failure({ error, shortcut }: { error: TranslateError | { kind: "nothingCaptured" }; shortcut: string }) {
  switch (error.kind) {
    case "nothingCaptured":
      return (
        <Notice title="原文を取り込めませんでした">
          <p>
            翻訳したい部分を選択してから <kbd>{shortcut}</kbd> を押してください。
          </p>
          <p>
            選択内容を自動で取り込むには、システム設定 › プライバシーとセキュリティ › アクセシビリティ で konnyaku
            を許可してください。許可していない場合や取り込めないアプリでは、先に <kbd>⌘C</kbd> でコピーしてから{" "}
            <kbd>{shortcut}</kbd> を押すとクリップボードの内容を翻訳します。
          </p>
        </Notice>
      );
    case "claudeNotFound":
      return (
        <Notice title="Claude Code が見つかりません">
          <p>
            Claude Code をインストールしてください。インストール済みの場合は、設定（<kbd>⌘,</kbd>）の「claude
            のパス」に実行ファイルの場所を入力してください。
          </p>
        </Notice>
      );
    case "notLoggedIn":
      return (
        <Notice title="Claude Code にログインしていません">
          <p>
            ターミナルで <code>claude</code> を実行し、<code>/login</code> でログインしてから、もう一度お試しください。
          </p>
        </Notice>
      );
    case "rateLimited":
      return (
        <Notice title="Claude の利用上限に達しました">
          <p>
            {error.resetsAt
              ? `${new Date(error.resetsAt).toLocaleString("ja-JP", { dateStyle: "short", timeStyle: "short" })} 以降にもう一度お試しください。`
              : "しばらく待ってから、もう一度お試しください。"}
          </p>
        </Notice>
      );
    case "processFailed":
      return (
        <Notice title="翻訳に失敗しました">
          <p>Claude Code が次のエラーで終了しました。</p>
          <pre className="stderr">{error.detail}</pre>
        </Notice>
      );
  }
}

function useShortcut(): string {
  const [settings, setSettings] = useState<Settings | null>(null);
  useEffect(() => {
    void getSettings().then(setSettings);
    const unlisten = onSettingsChanged(setSettings);
    return () => void unlisten.then((stop) => stop());
  }, []);
  return settings?.shortcut ?? "Control+J";
}
