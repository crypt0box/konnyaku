# デスクトップ基盤に Tauri v2（Rust + Web UI）を採用する

キャプチャにはコピー操作の擬似送信（macOS: CGEvent / Windows: SendInput）とクリップボード全形式の退避・復元（NSPasteboard / EnumClipboardFormats）が必要で、どの基盤でもネイティブ処理になる。Rust ならこれらと `claude` 子プロセスの管理を同一プロセス内で両 OS 分書けるうえ、常駐ツールとして軽量なため Tauri v2 を選んだ。Electron は TypeScript で完結する利点があるが、キー送信にネイティブアドオンか外部コマンドが要り、常駐には重い。OS ごとのネイティブ実装（Swift / C#）は OSS としての保守負担が大きいため採らなかった。
