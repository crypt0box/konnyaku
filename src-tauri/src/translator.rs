use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::thread;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;
use tempfile::TempDir;

use crate::settings::{Effort, Model};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum TranslateEvent {
    Delta { text: String },
    Done { truncated: bool },
    Error { error: TranslateError },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum TranslateError {
    ClaudeNotFound,
    NotLoggedIn,
    RateLimited {
        #[serde(skip_serializing_if = "Option::is_none")]
        resets_at: Option<String>,
    },
    ProcessFailed {
        detail: String,
    },
}

pub struct Request {
    pub claude_path: Option<String>,
    pub model: Model,
    pub effort: Effort,
    pub system_prompt: String,
    pub content: String,
}

struct Job {
    id: u64,
    child: Child,
    _cwd: TempDir,
}

impl Job {
    fn kill(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Default)]
struct Slot {
    /// Id of the newest start; every start and cancel bumps it, so an older job knows it was superseded.
    latest: u64,
    job: Option<Job>,
}

impl Slot {
    fn supersede(&mut self) -> u64 {
        self.latest += 1;
        if let Some(job) = self.job.take() {
            job.kill();
        }
        self.latest
    }
}

/// The single in-flight translation. Starting a job or cancelling kills the previous child.
#[derive(Default)]
pub struct Translator {
    slot: Arc<Mutex<Slot>>,
}

const STDERR_TAIL_BYTES: usize = 2000;

impl Translator {
    pub fn cancel(&self) {
        self.slot.lock().unwrap().supersede();
    }

    /// Returns at once: finding claude can wait on the login shell, and the caller may be the main thread.
    pub fn start(&self, request: Request, emit: impl Fn(TranslateEvent) + Send + 'static) {
        let id = self.slot.lock().unwrap().supersede();
        let slot = Arc::clone(&self.slot);
        thread::spawn(move || run(&slot, id, request, emit));
    }
}

fn run(slot: &Mutex<Slot>, id: u64, request: Request, emit: impl Fn(TranslateEvent)) {
    let Some(claude) = locate_claude(request.claude_path.as_deref()) else {
        emit(TranslateEvent::Error {
            error: TranslateError::ClaudeNotFound,
        });
        return;
    };
    let (mut child, cwd) = match spawn(&claude, &request) {
        Ok(spawned) => spawned,
        Err(error) => {
            emit(TranslateEvent::Error {
                error: spawn_error(error),
            });
            return;
        }
    };

    let mut stdin = child.stdin.take().expect("stdin is piped");
    let stdout = child.stdout.take().expect("stdout is piped");
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let job = Job {
        id,
        child,
        _cwd: cwd,
    };
    {
        let mut slot = slot.lock().unwrap();
        if slot.latest != id {
            drop(slot);
            return job.kill();
        }
        slot.job = Some(job);
    }

    let content = request.content;
    thread::spawn(move || {
        let _ = stdin.write_all(content.as_bytes());
    });
    let stderr_reader = thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = stderr.read_to_end(&mut buffer);
        String::from_utf8_lossy(&buffer).into_owned()
    });

    let mut stream = Stream::default();
    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        if let Some(text) = stream.apply(&line) {
            emit(TranslateEvent::Delta { text });
        }
    }
    let stderr = stderr_reader.join().unwrap_or_default();
    let Some(mut job) = take_job(slot, id) else {
        return;
    };
    let exited_ok = job.child.wait().map(|s| s.success()).unwrap_or(false);
    emit(stream.finish(exited_ok, tail(&stderr, STDERR_TAIL_BYTES)));
}

/// None when the job was cancelled or replaced, so a killed child never reports an error.
fn take_job(slot: &Mutex<Slot>, id: u64) -> Option<Job> {
    let mut slot = slot.lock().unwrap();
    if slot.job.as_ref().is_some_and(|job| job.id == id) {
        slot.job.take()
    } else {
        None
    }
}

fn spawn(claude: &Path, request: &Request) -> std::io::Result<(Child, TempDir)> {
    let cwd = tempfile::Builder::new().prefix("konnyaku-").tempdir()?;
    let mut command = Command::new(claude);
    command
        .args([
            "-p",
            "--output-format",
            "stream-json",
            "--verbose",
            "--include-partial-messages",
        ])
        .args([
            "--model",
            request.model.as_arg(),
            "--effort",
            request.effort.as_arg(),
        ])
        .args([
            "--tools",
            "",
            "--safe-mode",
            // Without this every 原文 and 訳文 lands in ~/.claude/projects as a resumable transcript.
            "--no-session-persistence",
            "--system-prompt",
            &request.system_prompt,
        ])
        .current_dir(cwd.path())
        // Overrides --effort when inherited from the user's environment.
        .env_remove("CLAUDE_CODE_EFFORT_LEVEL")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(path) = &login_shell().path {
        command.env("PATH", path);
    }
    Ok((command.spawn()?, cwd))
}

fn spawn_error(error: std::io::Error) -> TranslateError {
    match error.kind() {
        std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied => {
            TranslateError::ClaudeNotFound
        }
        _ => TranslateError::ProcessFailed {
            detail: error.to_string(),
        },
    }
}

/// State accumulated from `claude -p --output-format stream-json` lines.
#[derive(Default)]
struct Stream {
    stop_reason: Option<String>,
    result: Option<StreamResult>,
    auth_failed: bool,
    rate_limited: bool,
    resets_at: Option<String>,
}

struct StreamResult {
    is_error: bool,
    text: String,
}

impl Stream {
    /// Returns the text delta carried by the line, if any.
    fn apply(&mut self, line: &str) -> Option<String> {
        let line: Value = serde_json::from_str(line).ok()?;
        match line["type"].as_str()? {
            "stream_event" => {
                let event = &line["event"];
                match event["type"].as_str()? {
                    "content_block_delta" if event["delta"]["type"] == "text_delta" => {
                        return event["delta"]["text"].as_str().map(str::to_owned);
                    }
                    "message_delta" => {
                        if let Some(reason) = event["delta"]["stop_reason"].as_str() {
                            self.stop_reason = Some(reason.to_owned());
                        }
                    }
                    _ => {}
                }
            }
            "assistant" => match line["error"].as_str() {
                Some("authentication_failed") => self.auth_failed = true,
                Some("rate_limit") => self.rate_limited = true,
                _ => {}
            },
            "rate_limit_event" => {
                let info = &line["rate_limit_info"];
                if info["status"] == "rejected" {
                    self.rate_limited = true;
                    self.resets_at = info["resetsAt"]
                        .as_i64()
                        .and_then(|secs| chrono::DateTime::from_timestamp(secs, 0))
                        .map(|at| at.to_rfc3339());
                }
            }
            "result" => {
                self.result = Some(StreamResult {
                    is_error: line["is_error"].as_bool().unwrap_or(true),
                    text: line["result"].as_str().unwrap_or_default().to_owned(),
                });
            }
            _ => {}
        }
        None
    }

    fn finish(self, exited_ok: bool, stderr_tail: &str) -> TranslateEvent {
        let error = if self.auth_failed {
            TranslateError::NotLoggedIn
        } else if let Some(StreamResult {
            is_error: false, ..
        }) = self.result.as_ref().filter(|_| exited_ok)
        {
            return TranslateEvent::Done {
                truncated: self.stop_reason.as_deref() == Some("max_tokens"),
            };
        } else if self.rate_limited {
            TranslateError::RateLimited {
                resets_at: self.resets_at,
            }
        } else {
            let detail = [
                self.result.map(|r| r.text).unwrap_or_default(),
                stderr_tail.trim().to_owned(),
            ]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
            TranslateError::ProcessFailed {
                detail: if detail.is_empty() {
                    "claude exited without a result".into()
                } else {
                    detail
                },
            }
        };
        TranslateEvent::Error { error }
    }
}

fn tail(text: &str, max_bytes: usize) -> &str {
    let mut start = text.len().saturating_sub(max_bytes);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}

/// What the user's login shell knows. GUI apps do not inherit the shell's PATH.
struct LoginShell {
    path: Option<String>,
    claude: Option<PathBuf>,
}

fn login_shell() -> &'static LoginShell {
    static SHELL: OnceLock<LoginShell> = OnceLock::new();
    SHELL.get_or_init(|| {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        let script = format!(
            r#"printf '{PATH_MARK}%s{END_MARK}\n' "$PATH"; printf '{CLAUDE_MARK}%s{END_MARK}\n' "$(command -v claude)""#
        );
        let mut command = Command::new(shell);
        command.args(["-lc", &script]);
        LoginShell::parse(&stdout_within(command, LOGIN_SHELL_TIMEOUT).unwrap_or_default())
    })
}

/// A profile that hangs (a prompt, a network mount) must not hold every translation forever.
const LOGIN_SHELL_TIMEOUT: Duration = Duration::from_secs(5);

/// The command's stdout, or None when it cannot start or has not finished within `timeout` (it is killed then).
fn stdout_within(mut command: Command, timeout: Duration) -> Option<String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = stdout.read_to_end(&mut buffer);
        let _ = sender.send(String::from_utf8_lossy(&buffer).into_owned());
    });
    let output = receiver.recv_timeout(timeout).ok();
    if output.is_none() {
        let _ = child.kill();
    }
    let _ = child.wait();
    output
}

// Profile scripts may print anything, so the values are read only from between these marks.
const PATH_MARK: &str = "<<konnyaku-path>>";
const CLAUDE_MARK: &str = "<<konnyaku-claude>>";
const END_MARK: &str = "<<konnyaku-end>>";

impl LoginShell {
    fn parse(stdout: &str) -> LoginShell {
        let between = |mark: &str| {
            let start = stdout.rfind(mark)? + mark.len();
            let value = stdout[start..].split(END_MARK).next()?.trim();
            (!value.is_empty()).then(|| value.to_owned())
        };
        LoginShell {
            path: between(PATH_MARK),
            claude: between(CLAUDE_MARK).map(PathBuf::from),
        }
    }
}

/// Runs the login shell ahead of the first translation so it does not add to that translation's latency.
pub fn warm_up() {
    thread::spawn(login_shell);
}

pub fn locate_claude(override_path: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = override_path.map(str::trim).filter(|p| !p.is_empty()) {
        let path = PathBuf::from(path);
        return is_executable(&path).then_some(path);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    login_shell()
        .claude
        .clone()
        .into_iter()
        .chain([
            home.join(".local/bin/claude"),
            home.join(".claude/local/claude"),
            PathBuf::from("/opt/homebrew/bin/claude"),
            PathBuf::from("/usr/local/bin/claude"),
        ])
        .find(|p| is_executable(p))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(lines: &[&str], exited_ok: bool, stderr: &str) -> (String, TranslateEvent) {
        let mut stream = Stream::default();
        let text: String = lines.iter().filter_map(|line| stream.apply(line)).collect();
        (text, stream.finish(exited_ok, stderr))
    }

    const DELTA_A: &str = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"こんにちは"}}}"#;
    const DELTA_B: &str = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"世界"}}}"#;
    const SUCCESS: &str = r#"{"type":"result","subtype":"success","is_error":false,"result":"こんにちは世界","stop_reason":"end_turn"}"#;

    #[test]
    fn streams_text_and_finishes() {
        let stop = r#"{"type":"stream_event","event":{"type":"message_delta","delta":{"stop_reason":"end_turn"}}}"#;
        let (text, event) = run(
            &[
                r#"{"type":"system","subtype":"init"}"#,
                DELTA_A,
                DELTA_B,
                stop,
                SUCCESS,
            ],
            true,
            "",
        );
        assert_eq!(text, "こんにちは世界");
        assert_eq!(event, TranslateEvent::Done { truncated: false });
    }

    #[test]
    fn reports_truncation_at_max_tokens() {
        let stop = r#"{"type":"stream_event","event":{"type":"message_delta","delta":{"stop_reason":"max_tokens"}}}"#;
        let (_, event) = run(&[DELTA_A, stop, SUCCESS], true, "");
        assert_eq!(event, TranslateEvent::Done { truncated: true });
    }

    #[test]
    fn classifies_not_logged_in() {
        let assistant = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Not logged in · Please run /login"}]},"error":"authentication_failed","is_api_error_message":true}"#;
        let result = r#"{"type":"result","subtype":"success","is_error":true,"result":"Not logged in · Please run /login"}"#;
        let (text, event) = run(&[assistant, result], false, "");
        assert_eq!(text, "");
        assert_eq!(
            event,
            TranslateEvent::Error {
                error: TranslateError::NotLoggedIn
            }
        );
    }

    #[test]
    fn classifies_rate_limit_with_reset_time() {
        let limit = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":1795584000,"rateLimitType":"five_hour"}}"#;
        let result =
            r#"{"type":"result","subtype":"success","is_error":true,"result":"limit reached"}"#;
        let (_, event) = run(&[limit, result], false, "");
        assert_eq!(
            event,
            TranslateEvent::Error {
                error: TranslateError::RateLimited {
                    resets_at: Some("2026-11-25T05:20:00+00:00".into())
                }
            }
        );
    }

    #[test]
    fn allowed_rate_limit_event_is_not_an_error() {
        let limit = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","resetsAt":1795584000}}"#;
        let (_, event) = run(&[DELTA_A, limit, SUCCESS], true, "");
        assert_eq!(event, TranslateEvent::Done { truncated: false });
    }

    #[test]
    fn process_failure_carries_stderr_tail() {
        let (_, event) = run(&["not json"], false, "error: unknown option --safe-mode");
        assert_eq!(
            event,
            TranslateEvent::Error {
                error: TranslateError::ProcessFailed {
                    detail: "error: unknown option --safe-mode".into()
                }
            }
        );
    }

    /// These run real processes through /bin/sh, so they exist only where it does.
    #[cfg(unix)]
    mod processes {
        use std::time::Instant;

        use super::*;

        /// A fake claude that records its pid and waits to be killed.
        fn sleeping_claude(dir: &Path) -> Request {
            let pids = dir.join("pids");
            fake_claude(
                dir,
                &format!("echo $$ >> '{}'\nexec /bin/sleep 30", pids.display()),
            )
        }

        fn fake_claude(dir: &Path, body: &str) -> Request {
            use std::os::unix::fs::PermissionsExt;
            let script = dir.join("claude");
            std::fs::write(&script, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
            Request {
                claude_path: Some(script.display().to_string()),
                model: Model::Sonnet,
                effort: Effort::Low,
                system_prompt: String::new(),
                content: String::new(),
            }
        }

        fn alive(pid: &str) -> bool {
            Command::new("/bin/kill")
                .args(["-0", pid])
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success())
        }

        #[test]
        fn concurrent_starts_then_cancel_leave_no_claude_running() {
            let dir = tempfile::tempdir().unwrap();
            let translator = Arc::new(Translator::default());
            let events = Arc::new(Mutex::new(Vec::new()));
            for _ in 0..20 {
                let barrier = Arc::new(std::sync::Barrier::new(2));
                let starts: Vec<_> = (0..2)
                    .map(|_| {
                        let (translator, barrier, events) =
                            (translator.clone(), barrier.clone(), events.clone());
                        let request = sleeping_claude(dir.path());
                        thread::spawn(move || {
                            barrier.wait();
                            translator.start(request, move |e| events.lock().unwrap().push(e));
                        })
                    })
                    .collect();
                starts.into_iter().for_each(|t| t.join().unwrap());
            }
            thread::sleep(std::time::Duration::from_secs(2));
            translator.cancel();
            thread::sleep(std::time::Duration::from_secs(1));
            let pids = std::fs::read_to_string(dir.path().join("pids")).unwrap();
            let running: Vec<_> = pids.lines().filter(|pid| alive(pid)).collect();
            assert_eq!(running, Vec::<&str>::new(), "claude children left running");
            assert_eq!(
                *events.lock().unwrap(),
                vec![],
                "a killed job reported an event"
            );
        }

        #[test]
        fn cancel_before_claude_has_started_leaves_nothing_running() {
            let dir = tempfile::tempdir().unwrap();
            let translator = Translator::default();
            let events = Arc::new(Mutex::new(Vec::new()));
            let sink = events.clone();
            translator.start(sleeping_claude(dir.path()), move |e| {
                sink.lock().unwrap().push(e)
            });
            translator.cancel();
            thread::sleep(Duration::from_secs(2));
            let pids = std::fs::read_to_string(dir.path().join("pids")).unwrap_or_default();
            assert_eq!(pids.lines().filter(|pid| alive(pid)).count(), 0);
            assert_eq!(*events.lock().unwrap(), vec![]);
        }

        #[test]
        fn the_newest_start_streams_to_completion() {
            let dir = tempfile::tempdir().unwrap();
            let output = dir.path().join("output.jsonl");
            std::fs::write(&output, format!("{DELTA_A}\n{SUCCESS}\n")).unwrap();
            let translator = Translator::default();
            let (sender, receiver) = mpsc::channel();
            translator.start(sleeping_claude(dir.path()), |_| {});
            translator.start(
                fake_claude(dir.path(), &format!("cat '{}'", output.display())),
                move |e| sender.send(e).unwrap(),
            );
            let timeout = Duration::from_secs(10);
            assert_eq!(
                receiver.recv_timeout(timeout).unwrap(),
                TranslateEvent::Delta {
                    text: "こんにちは".into()
                }
            );
            assert_eq!(
                receiver.recv_timeout(timeout).unwrap(),
                TranslateEvent::Done { truncated: false }
            );
        }

        #[test]
        fn a_hanging_login_shell_is_abandoned_at_the_timeout() {
            let mut command = Command::new("/bin/sh");
            command.args(["-c", "echo partial; exec /bin/sleep 30"]);
            let started = Instant::now();
            assert_eq!(stdout_within(command, Duration::from_millis(300)), None);
            assert!(started.elapsed() < Duration::from_secs(2));

            let mut command = Command::new("/bin/sh");
            command.args(["-c", "echo done"]);
            assert_eq!(
                stdout_within(command, Duration::from_secs(5)).as_deref(),
                Some("done\n")
            );
        }
    }

    #[test]
    fn login_shell_values_survive_profile_output() {
        let stdout = "Welcome back\nnvm: using node 22\n\
            <<konnyaku-path>>/opt/homebrew/bin:/usr/bin<<konnyaku-end>>\n\
            <<konnyaku-claude>>/Users/me/.local/bin/claude<<konnyaku-end>>\nbye\n";
        let shell = LoginShell::parse(stdout);
        assert_eq!(shell.path.as_deref(), Some("/opt/homebrew/bin:/usr/bin"));
        assert_eq!(
            shell.claude,
            Some(PathBuf::from("/Users/me/.local/bin/claude"))
        );
    }

    #[test]
    fn login_shell_without_claude_or_output_yields_nothing() {
        let shell = LoginShell::parse(
            "<<konnyaku-path>>/usr/bin<<konnyaku-end>>\n<<konnyaku-claude>><<konnyaku-end>>\n",
        );
        assert_eq!(shell.path.as_deref(), Some("/usr/bin"));
        assert_eq!(shell.claude, None);
        let silent = LoginShell::parse("");
        assert_eq!((silent.path, silent.claude), (None, None));
    }

    #[test]
    fn serializes_to_the_frontend_contract() {
        let json = |e: &TranslateEvent| serde_json::to_string(e).unwrap();
        assert_eq!(
            json(&TranslateEvent::Delta { text: "あ".into() }),
            r#"{"kind":"delta","text":"あ"}"#
        );
        assert_eq!(
            json(&TranslateEvent::Done { truncated: true }),
            r#"{"kind":"done","truncated":true}"#
        );
        assert_eq!(
            json(&TranslateEvent::Error {
                error: TranslateError::RateLimited {
                    resets_at: Some("t".into())
                }
            }),
            r#"{"kind":"error","error":{"kind":"rateLimited","resetsAt":"t"}}"#
        );
        assert_eq!(
            json(&TranslateEvent::Error {
                error: TranslateError::ClaudeNotFound
            }),
            r#"{"kind":"error","error":{"kind":"claudeNotFound"}}"#
        );
    }
}
