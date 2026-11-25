use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

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

/// The single in-flight translation. Starting a job or cancelling kills the previous child.
#[derive(Default)]
pub struct Translator {
    slot: Arc<Mutex<Option<Job>>>,
    next_id: AtomicU64,
}

const STDERR_TAIL_BYTES: usize = 2000;

impl Translator {
    pub fn cancel(&self) {
        if let Some(mut job) = self.slot.lock().unwrap().take() {
            let _ = job.child.kill();
            let _ = job.child.wait();
        }
    }

    pub fn start(&self, request: Request, emit: impl Fn(TranslateEvent) + Send + 'static) {
        self.cancel();
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
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        *self.slot.lock().unwrap() = Some(Job {
            id,
            child,
            _cwd: cwd,
        });

        let content = request.content;
        thread::spawn(move || {
            let _ = stdin.write_all(content.as_bytes());
        });
        let stderr_reader = thread::spawn(move || {
            let mut buffer = Vec::new();
            let _ = stderr.read_to_end(&mut buffer);
            String::from_utf8_lossy(&buffer).into_owned()
        });

        let slot = Arc::clone(&self.slot);
        thread::spawn(move || {
            let mut stream = Stream::default();
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if let Some(text) = stream.apply(&line) {
                    emit(TranslateEvent::Delta { text });
                }
            }
            let stderr = stderr_reader.join().unwrap_or_default();
            let Some(mut job) = take_job(&slot, id) else {
                return;
            };
            let exited_ok = job.child.wait().map(|s| s.success()).unwrap_or(false);
            emit(stream.finish(exited_ok, tail(&stderr, STDERR_TAIL_BYTES)));
        });
    }
}

/// None when the job was cancelled or replaced, so a killed child never reports an error.
fn take_job(slot: &Mutex<Option<Job>>, id: u64) -> Option<Job> {
    let mut slot = slot.lock().unwrap();
    if slot.as_ref().is_some_and(|job| job.id == id) {
        slot.take()
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
        let output = Command::new(shell)
            .args(["-lc", r#"printf '%s\n' "$PATH"; command -v claude"#])
            .stdin(Stdio::null())
            .output();
        let stdout = output
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        let mut lines = stdout.lines().map(str::trim).filter(|l| !l.is_empty());
        LoginShell {
            path: lines.next().map(str::to_owned),
            claude: lines.next().map(PathBuf::from),
        }
    })
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
