#!/bin/bash
# Drives the signed dev build (scripts/build-dev-app.sh) through its GUI behaviors with synthetic
# input posted by the app itself (src-tauri/src/debug.rs), since the app holds the Accessibility
# grant and the calling shell does not. Uses TextEdit and Safari as 原文 sources and quits them at
# the end if this script launched them. Restores the user's clipboard, settings and window state.
# Usage: scripts/e2e/gui-verify.sh [out-dir]
set -u
here=$(cd "$(dirname "$0")" && pwd)
bundle="$here/../../src-tauri/target/debug/bundle/macos/konnyaku.app"
support="$HOME/Library/Application Support/dev.konnyaku.app"
socket=/tmp/dev_konnyaku_app_si.sock
out=${1:-$(mktemp -d)}
mkdir -p "$out/backup"
dump="$out/rendered.html"
passed=0
failed=0

swiftc -O "$here/pbtool.swift" -o "$out/pbtool" || exit 1
swiftc -O "$here/winid.swift" -o "$out/winid" || exit 1

# lsappinfo, not pgrep: a quit TextEdit can linger as a process under automatic termination.
running_app() { [ -n "$(lsappinfo info -app "$1")" ]; }
sources_launched=""
for app in TextEdit Safari; do running_app "$app" || sources_launched="$sources_launched $app"; done

if pgrep -f "konnyaku.app/Contents/MacOS/konnyaku" >/dev/null; then
  echo "a konnyaku instance is running; quit it first (single-instance would forward to it)" >&2
  exit 1
fi

"$out/pbtool" save "$out/backup/clipboard.plist" >/dev/null
for f in settings.json .window-state.json; do
  [ -f "$support/$f" ] && cp -p "$support/$f" "$out/backup/$f"
done

# Quits the 原文 apps this script launched; needs a running konnyaku to post Cmd+Q.
quit_sources() {
  for app in $sources_launched; do
    running_app "$app" && activate "$app" && keys cmd+q
  done
  sources_launched=""
}

cleanup() {
  if [ -n "$(app_pid)" ]; then
    quit_sources
    pkill -f "konnyaku.app/Contents/MacOS/konnyaku"
    sleep 1
  fi
  [ -n "$sources_launched" ] && echo "left running:$sources_launched"
  "$out/pbtool" restore "$out/backup/clipboard.plist" >/dev/null
  for f in settings.json .window-state.json; do
    if [ -f "$out/backup/$f" ]; then cp -p "$out/backup/$f" "$support/$f"; else rm -f "$support/$f"; fi
  done
  echo "restored clipboard, settings and window state; $passed passed, $failed failed"
}
trap cleanup EXIT

pass() { passed=$((passed + 1)); echo "PASS $1: $2"; }
fail() { failed=$((failed + 1)); echo "FAIL $1: $2"; }
check() { local name=$1 detail=$2; shift 2; if "$@"; then pass "$name" "$detail"; else fail "$name" "$detail"; fi; }

send() {
  python3 -c 'import socket, sys
s = socket.socket(socket.AF_UNIX)
s.connect(sys.argv[1])
s.sendall(("/\0\0" + "\0".join(["konnyaku"] + sys.argv[2:])).encode())
s.close()' "$socket" "$@"
}
keys() { send --debug-keys "$1"; sleep 0.3; }

# Runs a JS function body in a webview (main or settings) and prints its JSON result.
js() {
  local result="$out/eval.json"
  rm -f "$result"
  send --debug-eval "$1" "$result" "$2"
  wait_until 10 test -s "$result" && cat "$result"
}
js_value() { js "$1" "$2" | python3 -c 'import json, sys; r = json.load(sys.stdin); v = r.get("value")
print(("ERROR " + r["error"]) if not r["ok"] else v if isinstance(v, str) else json.dumps(v))'; }

wait_until() {
  local deadline=$(($(date +%s) + $1))
  shift
  until "$@"; do
    [ "$(date +%s)" -ge "$deadline" ] && return 1
    sleep 0.1
  done
}

app_pid() { pgrep -f "konnyaku.app/Contents/MacOS/konnyaku" | head -1; }
front_name() { lsappinfo info -only name "$(lsappinfo front)" | sed 's/.*="\(.*\)"/\1/'; }
front_app() { [ "$(front_name)" = "$1" ]; }
activate() { open -a "$1" ${2:+"$2"} && wait_until 10 front_app "$1" && sleep 0.8; }
window() { "$out/winid" | awk -v t="$1" '$2 == t'; }
visible() { [ -n "$(window "$1")" ]; }
hidden() { [ -z "$(window "$1")" ]; }
bounds() { window "$1" | awk '{ print $3, $4, $5, $6 }'; }
translations() { pgrep -P "$(app_pid)" -f stream-json; }
has_translation() { [ -n "$(translations)" ]; }
no_translation() { [ -z "$(translations)" ]; }
gone() { ! kill -0 "$1" 2>/dev/null; }
has_dump() { test -s "$dump"; }
contains() { grep -q -- "$2" "$1"; }
setting() { python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["settings"][sys.argv[2]])' "$support/settings.json" "$1"; }

launch() {
  rm -f "$dump"
  open -n --env KONNYAKU_DUMP="$dump" "$bundle"
  wait_until 15 test -S "$socket" && wait_until 15 visible konnyaku && sleep 1
}

click_content() {
  local x y w h
  read -r x y w h <<<"$("$out/winid" "$1" | head -1 | awk '{ print $3, $4, $5, $6 }')"
  send --debug-click $((x + w / 2)) $((y + h * 2 / 3))
  sleep 0.3
}

# Clicks into the front window's content, selects everything, and presses the shortcut.
translate_selection() {
  rm -f "$dump"
  activate "$1" ${2:+"$2"}
  click_content "$1"
  keys cmd+a
  keys "${3:-ctrl+j}"
}

textutil -convert rtf -output "$out/short.rtf" "$here/textedit.html"
{
  echo '<meta charset="utf-8"><h1>Field Notes from a Long Walk</h1>'
  for i in $(seq 1 30); do
    echo "<p>Day $i. We left camp before sunrise and followed the river for several hours, stopping only to refill our bottles and to watch the herons fish in the shallows. The afternoon brought wind and a light rain, so we pitched the tents early and spent the evening repairing boots and talking about the next pass.</p>"
  done
} >"$out/long.html"
textutil -convert rtf -output "$out/long.rtf" "$out/long.html"
cp "$here/page.html" "$out/page.html"

cat >"$support/settings.json" <<'EOF'
{"settings":{"claudePath":null,"effort":"low","launchAtLogin":true,"model":"sonnet","shortcut":"Control+J","style":"followSource"}}
EOF

launch || { echo "the app did not start"; exit 1; }
rm -f "$out/ax.json"
send --debug-ax "$out/ax.json"
wait_until 5 test -s "$out/ax.json"
if ! contains "$out/ax.json" '"trusted":true'; then
  echo "FAIL preflight: the app is not Accessibility trusted ($(cat "$out/ax.json" 2>/dev/null)); grant it and rerun"
  exit 1
fi
pass preflight "AXIsProcessTrusted() is true in pid $(app_pid)"

"$out/pbtool" sentinel >/dev/null
"$out/pbtool" digest >"$out/clip-before.txt"
translate_selection TextEdit "$out/short.rtf"
wait_until 90 has_dump
"$out/pbtool" digest >"$out/clip-after.txt"
cp "$dump" "$out/1a-textedit.html"
check 1a-textedit "訳文 rendered from the TextEdit selection (ZX-81 present, not the clipboard sentinel)" \
  eval 'contains "$dump" ZX-81 && ! contains "$dump" sentinel'
structure=$(js_value main 'const a = document.querySelector("article"); return [/[ぁ-ん]/.test(a.innerText), a.querySelectorAll("li").length, a.querySelectorAll("table tr").length].join(",")')
check 1a-textedit "訳文 is Japanese and keeps the list and table (kana,li,tr = $structure, want true,2,3)" test "$structure" = "true,2,3"
echo "info 1a-textedit: headings in 訳文: $(js_value main 'return document.querySelectorAll("article h1,article h2,article h3").length') (RTF carries no heading semantics)"
check 1a-clipboard "clipboard byte-identical after the capture ($(wc -l <"$out/clip-before.txt" | tr -d ' ') item/type digests)" \
  cmp -s "$out/clip-before.txt" "$out/clip-after.txt"

"$out/pbtool" sentinel >/dev/null
"$out/pbtool" digest >"$out/clip-before.txt"
translate_selection Safari "$out/page.html"
wait_until 90 has_dump
"$out/pbtool" digest >"$out/clip-after.txt"
sleep 3
cp "$dump" "$out/1b-safari.html"
check 1b-safari "訳文 rendered from the Safari selection (inline code page_marker_b restored verbatim)" \
  test "$(js_value main 'return document.querySelector("article p code")?.textContent ?? ""')" = page_marker_b
structure=$(js_value main 'const a = document.querySelector("article"); return [/[ぁ-ん]/.test(a.innerText), a.querySelectorAll("h1").length, a.querySelectorAll("h2").length, a.querySelectorAll("li").length, a.querySelectorAll("table tr").length, a.querySelector("pre code")?.textContent.trim()].join(",")')
check 1b-safari "訳文 is Japanese and keeps h1, h2, list, table, code block (got $structure)" \
  test "$structure" = "true,1,1,3,3,checklist --trail north-ridge --days 2"
image=$(js_value main 'const img = document.querySelector("article img"); return img ? `${img.complete && img.naturalWidth > 0}:${img.getAttribute("src")}` : "none"')
check 1b-image "public image loads in the 訳文 ($image)" eval '[[ $image == true:https://www.rust-lang.org/static/images/rust-logo-blk.svg ]]'
box=$(js_value main 'const b = document.querySelector(".unloadable-image"); return b ? `${b.querySelector(".unloadable-image-alt")?.textContent}|${b.querySelector("a")?.textContent}|${b.querySelector("a")?.getAttribute("href")}` : "none"')
check 1b-image "failed image became the alt + ブラウザで開く box ($box)" \
  test "$box" = "Missing route sketch|ブラウザで開く|https://www.rust-lang.org/this-image-does-not-exist.png"
check 1b-clipboard "clipboard byte-identical after the capture" cmp -s "$out/clip-before.txt" "$out/clip-after.txt"

"$out/pbtool" sentinel >/dev/null
"$out/pbtool" digest >"$out/clip-before.txt"
rm -f "$dump"
activate TextEdit "$out/short.rtf"
click_content TextEdit
keys cmd+a
send --debug-keys ctrl+j 2
wait_until 90 has_dump
sleep 1
"$out/pbtool" digest >"$out/clip-after.txt"
cp "$dump" "$out/1c-double-press.html"
check 1c-double-press "two quick presses translate the TextEdit selection (ZX-81 present, not the clipboard sentinel)" \
  eval 'contains "$dump" ZX-81 && ! contains "$dump" sentinel'
check 1c-double-press "clipboard byte-identical after two quick presses" cmp -s "$out/clip-before.txt" "$out/clip-after.txt"

translate_selection TextEdit "$out/long.rtf"
if wait_until 15 has_translation && wait_until 10 front_app konnyaku; then
  child=$(translations)
  sleep 1
  grandchildren=$(pgrep -P "$child" | tr '\n' ' ')
  keys escape
  wait_until 5 gone "$child"
  check 2-escape "claude child $child killed by Esc" gone "$child"
  for g in $grandchildren; do check 2-escape "claude's own child $g gone too" wait_until 3 gone "$g"; done
  check 2-escape "no claude child left" no_translation
  check 2-escape "翻訳ウィンドウ hidden" wait_until 3 hidden konnyaku
  sleep 3
  check 2-escape "the cancelled job never rendered" eval '! has_dump'
else
  fail 2-escape "no in-flight claude child appeared (front: $(front_name))"
fi

translate_selection TextEdit "$out/long.rtf"
if wait_until 15 has_translation; then
  first=$(translations)
  rm -f "$out/children.log"
  (while [ ! -f "$out/stop-monitor" ]; do translations | wc -l | tr -d ' ' >>"$out/children.log"; sleep 0.02; done) &
  monitor=$!
  activate Safari "$out/page.html"
  click_content Safari
  keys cmd+a
  check 6-retrigger "first job still in flight when the shortcut is pressed again" eval 'kill -0 "$first" && ! has_dump'
  keys ctrl+j
  wait_until 90 has_dump
  touch "$out/stop-monitor"
  wait "$monitor"
  rm -f "$out/stop-monitor"
  check 6-retrigger "first claude child $first killed" gone "$first"
  most=$(sort -n "$out/children.log" | tail -1)
  check 6-retrigger "never two claude children ($(wc -l <"$out/children.log" | tr -d ' ') samples, max $most)" test "$most" -le 1
  check 6-retrigger "window ends with the second content (page_marker_b, no Day N)" \
    eval 'contains "$dump" page_marker_b && ! contains "$dump" "Day 1"'
else
  fail 6-retrigger "no in-flight claude child appeared"
fi

pid=$(app_pid)
read -r x y w h <<<"$(bounds konnyaku)"
send --debug-click $((x + 16)) $((y + 16))
check 3-close "red close button hides the 翻訳ウィンドウ" wait_until 3 hidden konnyaku
check 3-close "app keeps running (pid $pid)" kill -0 "$pid"
translate_selection Safari "$out/page.html"
check 3-close "Control+J still translates after closing" wait_until 90 has_dump
check 3-close "翻訳ウィンドウ shown again" visible konnyaku

before=$(js_value main 'return document.getElementById("content").innerText')
read -r x y w h <<<"$(bounds konnyaku)"
send --debug-click $((x + 16)) $((y + 16))
wait_until 3 hidden konnyaku
open "$bundle"
check 4-reopen "opening the bundle again shows the 翻訳ウィンドウ" wait_until 5 visible konnyaku
after=$(js_value main 'return document.getElementById("content").innerText')
check 4-reopen "same pid, so no second instance" test "$(app_pid)" = "$pid"
check 4-reopen "window shows the last translation (${#after} chars)" eval '[ -n "$before" ] && [ "$before" = "$after" ] && [[ $after == *page_marker_b* ]]'

keys cmd+comma
check 7-settings "Cmd+, opens the settings window" wait_until 5 visible 設定
js settings 'const pick = async (i, value) => {
  const select = document.querySelectorAll("select")[i];
  select.value = value;
  select.dispatchEvent(new Event("change", { bubbles: true }));
  await new Promise((r) => setTimeout(r, 300));
};
await pick(0, "haiku"); await pick(1, "medium"); await pick(2, "desuMasu");' >/dev/null
saved="$(setting model) $(setting effort) $(setting style)"
check 7-settings "model, effort, style saved ($saved)" test "$saved" = "haiku medium desuMasu"
translate_selection Safari "$out/page.html"
if wait_until 15 has_translation; then
  argv=$(ps -o command= -p "$(translations)")
  check 7-settings "next claude spawn uses --model haiku --effort medium" eval '[[ $argv == *"--model haiku --effort medium"* ]]'
  check 7-settings "next claude spawn carries the です・ます style rule" eval '[[ $argv == *"Write in です・ます調 throughout."* ]]'
else
  fail 7-settings "no claude child after changing settings"
fi
wait_until 90 has_dump

keys cmd+comma
wait_until 5 visible 設定
focused=$(js_value settings 'const input = document.querySelector("input[readonly]"); input.focus(); return document.activeElement === input')
keys ctrl+shift+k
sleep 0.5
check 7-shortcut "shortcut field recorded the pressed keys (focused $focused, saved $(setting shortcut))" test "$(setting shortcut)" = Control+Shift+K
translate_selection Safari "$out/page.html" ctrl+j
sleep 3
check 7-shortcut "old Control+J no longer triggers (front $(front_name))" eval '! has_dump && no_translation && front_app Safari'
keys ctrl+shift+k
check 7-shortcut "new Control+Shift+K triggers a translation" wait_until 90 has_dump

translate_selection TextEdit "$out/long.rtf" ctrl+shift+k
if wait_until 15 has_translation && wait_until 10 front_app konnyaku; then
  child=$(translations)
  cwd=$(lsof -a -p "$child" -d cwd -Fn | sed -n 's/^n//p')
  pid=$(app_pid)
  keys cmd+q
  check 5-quit-translating "Cmd+Q during a translation quits (pid $pid gone)" wait_until 5 gone "$pid"
  check 5-quit-translating "claude child $child killed on quit" wait_until 3 gone "$child"
  check 5-quit-translating "claude's temp dir removed on quit ($cwd)" eval '[[ $cwd == */konnyaku-* ]] && [ ! -e "$cwd" ]'
  launch
else
  fail 5-quit-translating "no in-flight claude child appeared (front: $(front_name))"
fi

keys cmd+comma
wait_until 5 visible 設定
js settings 'const input = document.querySelector("input[placeholder]");
input.focus(); input.value = "/nonexistent/claude"; input.blur();
await new Promise((r) => setTimeout(r, 300));' >/dev/null
check 7-claude-path "invalid claude path saved ($(setting claudePath))" test "$(setting claudePath)" = /nonexistent/claude
translate_selection Safari "$out/page.html" ctrl+shift+k
wait_until 30 has_dump
check 7-claude-path "翻訳ウィンドウ shows the not-found error" contains "$dump" "Claude Code が見つかりません"

read -r x y w h <<<"$(bounds konnyaku)"
send --debug-drag $((x + w / 2)) $((y + 14)) $((x + w / 2 + 90)) $((y + 14 + 60))
sleep 0.5
read -r x2 y2 w2 h2 <<<"$(bounds konnyaku)"
send --debug-drag $((x2 + w2 - 2)) $((y2 + h2 - 2)) $((x2 + w2 + 48)) $((y2 + h2 + 38))
sleep 0.5
moved=$(bounds konnyaku)
check 8-frame "drags moved and resized the 翻訳ウィンドウ ($x $y $w $h -> $moved)" eval 'read -r a b c d <<<"$moved"; [ "$a" != "$x" ] && [ "$b" != "$y" ] && [ "$c" != "$w" ] && [ "$d" != "$h" ]'

quit_sources
pid=$(app_pid)
open "$bundle"
wait_until 5 front_app konnyaku
keys cmd+q
check 5-quit "Cmd+Q quits (pid $pid gone)" wait_until 5 gone "$pid"

launch
check 8-frame "frame restored after restart ($(bounds konnyaku), want $moved)" test "$(bounds konnyaku)" = "$moved"
restored=$(js_value main 'const s = await window.__TAURI_INTERNALS__.invoke("get_settings"); return [s.model, s.effort, s.style, s.shortcut, s.claudePath].join(" ")')
check 7-persist "settings survive a restart ($restored)" test "$restored" = "haiku medium desuMasu Control+Shift+K /nonexistent/claude"
pid=$(app_pid)
keys cmd+q
check 5-quit "Cmd+Q quits again (pid $pid gone)" wait_until 5 gone "$pid"

[ "$failed" -eq 0 ]
