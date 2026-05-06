use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

use persona_message::schema::Message;

const RECEIVER_READY_TIMEOUT: Duration = Duration::from_secs(180);
const SENDER_TIMEOUT: Duration = Duration::from_secs(240);
const RECEIVER_ACK_TIMEOUT: Duration = Duration::from_secs(240);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HarnessKind {
    Codex,
    Claude,
}

impl HarnessKind {
    fn program(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }

    fn actor(self) -> &'static str {
        match self {
            Self::Codex => "operator",
            Self::Claude => "designer",
        }
    }

    fn model(self) -> &'static str {
        match self {
            Self::Codex => "gpt-5.4-mini",
            Self::Claude => "claude-haiku-4-5",
        }
    }

    fn ready_marker(self) -> &'static str {
        match self {
            Self::Codex => "READY-OPERATOR",
            Self::Claude => "READY-DESIGNER",
        }
    }

    fn received_marker(self) -> &'static str {
        match self {
            Self::Codex => "RECEIVED-OPERATOR",
            Self::Claude => "RECEIVED-DESIGNER",
        }
    }
}

#[derive(Debug, Clone)]
struct Pane {
    id: u64,
    pid_file: PathBuf,
    transcript: PathBuf,
}

#[derive(Debug)]
struct ActualHarnessFixture {
    root: PathBuf,
    store: PathBuf,
    workspace: PathBuf,
    panes: Vec<Pane>,
    keep_root: bool,
}

impl ActualHarnessFixture {
    fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let root = std::env::temp_dir().join(format!(
            "persona-message-actual-harness-{}-{timestamp}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("temporary root directory");
        eprintln!("actual harness temp root: {}", root.display());
        let store = root.join("store");
        let workspace = root.join("workspace");
        fs::create_dir_all(&store).expect("store directory");
        fs::create_dir_all(&workspace).expect("workspace directory");
        Self {
            root,
            store,
            workspace,
            panes: Vec::new(),
            keep_root: std::env::var("PERSONA_KEEP_ACTUAL_HARNESS_TMP").as_deref() == Ok("1"),
        }
    }

    fn spawn(&mut self, kind: HarnessKind) -> Pane {
        let actor_dir = self.workspace.join(kind.actor());
        fs::create_dir_all(&actor_dir).expect("actor workspace");
        let pid_file = self.root.join(format!("{}.pid", kind.actor()));
        let transcript = self.root.join(format!("{}.typescript", kind.actor()));
        let message_bin = PathBuf::from(env!("CARGO_BIN_EXE_message"));
        let message_dir = message_bin.parent().expect("message binary parent");
        let runner_path = format!(
            "{}:{}",
            message_dir.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let harness_command = shell_exec_for_harness(kind, &actor_dir);
        let runner = self.root.join(format!("run-{}.sh", kind.actor()));
        fs::write(
            &runner,
            format!(
                "#!/bin/sh\nset -eu\nprintf '%s\\n' $$ > '{}'\nexport PERSONA_MESSAGE_STORE='{}'\nexport PATH={}\nunset NO_COLOR\nexport TERM=xterm-256color\nexport COLORTERM=truecolor\nexport CLICOLOR=1\nexport FORCE_COLOR=1\ncommand -v {} >/dev/null\n{}\n",
                pid_file.display(),
                self.store.display(),
                shell_quote(&runner_path),
                kind.program(),
                harness_command,
            ),
        )
        .expect("runner script writes");
        let shell = format!(
            "chmod +x '{}'; exec '{}'",
            runner.display(),
            runner.display(),
        );
        eprintln!("spawning {} pane with {}", kind.actor(), kind.program());
        let pane = self.wezterm_spawn(&["sh", "-lc", &shell]);
        let pane = Pane {
            id: pane,
            pid_file,
            transcript,
        };
        self.panes.push(pane.clone());
        self.dismiss_startup_dialogs(kind, &pane);
        pane
    }

    fn dismiss_startup_dialogs(&self, kind: HarnessKind, pane: &Pane) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            let text = match self.try_get_text(pane) {
                Ok(text) => text,
                Err(error) => {
                    let transcript = fs::read_to_string(&pane.transcript).unwrap_or_default();
                    panic!(
                        "failed to read pane {} while dismissing startup dialogs: {error}\ntranscript:\n{transcript}",
                        pane.id
                    );
                }
            };
            if text.contains("Quick safety check")
                || text.contains("Yes, I trust this folder")
                || text.contains("Enter to confirm")
            {
                eprintln!("dismissing startup trust dialog for {}", kind.actor());
                self.wezterm([
                    "send-text",
                    "--pane-id",
                    &pane.id.to_string(),
                    "--no-paste",
                    "\r",
                ]);
                return;
            }
            if text.contains(kind.ready_marker()) {
                return;
            }
            thread::sleep(Duration::from_millis(500));
        }
    }

    fn write_agents(&self, agents: &[(HarnessKind, &Pane)]) {
        let mut lines = Vec::new();
        for (kind, pane) in agents {
            let pid = wait_for_file(&pane.pid_file, Duration::from_secs(20));
            lines.push(format!(
                "(Actor {} {} (EndpointTransport wezterm-pane \"{}\" None))",
                kind.actor(),
                pid.trim(),
                pane.id
            ));
        }
        fs::write(
            self.store.join("actors.nota"),
            format!("{}\n", lines.join("\n")),
        )
        .expect("actor index writes");
    }

    fn wait_for_text(&self, pane: &Pane, needle: &str, timeout: Duration) -> String {
        let deadline = Instant::now() + timeout;
        let mut last = String::new();
        while Instant::now() < deadline {
            let transcript = fs::read_to_string(&pane.transcript).unwrap_or_default();
            if transcript.contains(needle) {
                return transcript;
            }
            last = self
                .try_get_text(pane)
                .unwrap_or_else(|error| format!("wezterm get-text failed: {error}"));
            if last.contains(needle) {
                return last;
            }
            thread::sleep(Duration::from_millis(750));
        }
        let transcript = fs::read_to_string(&pane.transcript).unwrap_or_default();
        panic!(
            "pane {} did not contain {needle:?}; last text:\n{last}\ntranscript:\n{transcript}",
            pane.id
        );
    }

    fn wait_for_message(&self, body: &str, timeout: Duration) -> Message {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            for message in self.messages() {
                if message.body == body {
                    return message;
                }
            }
            thread::sleep(Duration::from_millis(750));
        }
        panic!("message body {body:?} was not appended");
    }

    fn messages(&self) -> Vec<Message> {
        let path = self.store.join("messages.nota.log");
        let Ok(text) = fs::read_to_string(path) else {
            return Vec::new();
        };
        text.lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| Message::from_nota(line).expect("message log line decodes"))
            .collect()
    }

    fn nudge(&self, pane: &Pane, text: &str) {
        self.wezterm(["send-text", "--pane-id", &pane.id.to_string(), text]);
        thread::sleep(Duration::from_millis(500));
        self.wezterm([
            "send-text",
            "--pane-id",
            &pane.id.to_string(),
            "--no-paste",
            "\r",
        ]);
    }

    fn try_get_text(&self, pane: &Pane) -> std::result::Result<String, String> {
        self.try_wezterm_output(["get-text", "--pane-id", &pane.id.to_string()])
    }

    fn wezterm_spawn(&self, command: &[&str]) -> u64 {
        let mut args = vec![
            "spawn".to_string(),
            "--new-window".to_string(),
            "--workspace".to_string(),
            "persona-message-actual-harness".to_string(),
            "--cwd".to_string(),
            self.workspace.display().to_string(),
            "--".to_string(),
        ];
        args.extend(command.iter().map(|part| (*part).to_string()));
        let output = self.wezterm_output(args);
        output.trim().parse().expect("pane id parses")
    }

    fn wezterm<I, S>(&self, args: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let output = self.wezterm_command(args).output().expect("wezterm runs");
        if !output.status.success() {
            panic!(
                "wezterm failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    fn wezterm_output<I, S>(&self, args: I) -> String
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let output = self.wezterm_command(args).output().expect("wezterm runs");
        if !output.status.success() {
            panic!(
                "wezterm failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        String::from_utf8(output.stdout).expect("wezterm stdout is utf8")
    }

    fn try_wezterm_output<I, S>(&self, args: I) -> std::result::Result<String, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let output = self
            .wezterm_command(args)
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).to_string());
        }
        String::from_utf8(output.stdout).map_err(|error| error.to_string())
    }

    fn wezterm_command<I, S>(&self, args: I) -> Command
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let mut command = Command::new(
            std::env::var("PERSONA_WEZTERM").unwrap_or_else(|_| "wezterm".to_string()),
        );
        command
            .arg("cli")
            .arg("--prefer-mux")
            .args(args)
            .env("PERSONA_MESSAGE_STORE", &self.store);
        command
    }
}

impl Drop for ActualHarnessFixture {
    fn drop(&mut self) {
        for pane in &self.panes {
            let _ = self
                .wezterm_command(["kill-pane", "--pane-id", &pane.id.to_string()])
                .output();
        }
        if !self.keep_root && !std::thread::panicking() {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn shell_exec_for_harness(kind: HarnessKind, cwd: &Path) -> String {
    match kind {
        HarnessKind::Codex => format!(
            "exec codex \\\n  --dangerously-bypass-approvals-and-sandbox \\\n  --model {} \\\n  -c {} \\\n  --cd {}",
            shell_quote(
                &std::env::var("PERSONA_CODEX_MODEL").unwrap_or_else(|_| kind.model().to_string())
            ),
            shell_quote("model_reasoning_effort=\"low\""),
            shell_quote(&cwd.display().to_string()),
        ),
        HarnessKind::Claude => format!(
            "exec claude \\\n  --model {} \\\n  --effort low \\\n  --dangerously-skip-permissions \\\n  --add-dir {}",
            shell_quote(
                &std::env::var("PERSONA_CLAUDE_MODEL").unwrap_or_else(|_| kind.model().to_string())
            ),
            shell_quote(&cwd.display().to_string()),
        ),
    }
}

fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

fn wait_for_file(path: &Path, timeout: Duration) -> String {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(text) = fs::read_to_string(path) {
            if !text.trim().is_empty() {
                return text;
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
    panic!("file was not written: {}", path.display());
}

fn require_actual_harnesses() {
    if std::env::var("PERSONA_RUN_ACTUAL_HARNESSES").as_deref() != Ok("1") {
        eprintln!("set PERSONA_RUN_ACTUAL_HARNESSES=1 to run authenticated harness tests");
        return;
    }
    for program in ["wezterm", "codex", "claude"] {
        let output = Command::new("sh")
            .arg("-lc")
            .arg(format!("command -v {program}"))
            .output()
            .expect("which command runs");
        assert!(
            output.status.success(),
            "{program} must be available in PATH"
        );
    }
}

fn receiver_prompt(receiver: HarnessKind) -> String {
    format!(
        "Persona message test for {actor}. Reply exactly {ready}. Then wait idle. \
         If terminal input contains an incoming (Message ...) NOTA record addressed to {actor} \
         whose body begins with REPLY_REQUEST:, read the from field from the record, \
         run: message '(Send <from> \"REPLY_RESPONSE:reply from {actor}\")', \
         do not auto-reply to REPLY_RESPONSE messages, \
         then reply exactly {received}-FROM-<from>.",
        actor = receiver.actor(),
        ready = receiver.ready_marker(),
        received = receiver.received_marker(),
    )
}

fn sender_prompt(sender: HarnessKind, receiver: HarnessKind, body: &str) -> String {
    format!(
        "Persona message test for {sender}. Run this shell command exactly: message '(Send {receiver} \"{body}\")'. Then reply exactly SENT-{sender_upper}.",
        sender = sender.actor(),
        receiver = receiver.actor(),
        body = body,
        sender_upper = sender.actor().to_ascii_uppercase(),
    )
}

fn run_initiator_test(sender: HarnessKind, receiver: HarnessKind, body: &str) {
    require_actual_harnesses();
    if std::env::var("PERSONA_RUN_ACTUAL_HARNESSES").as_deref() != Ok("1") {
        return;
    }

    let mut fixture = ActualHarnessFixture::new();
    let receiver_pane = fixture.spawn(receiver);
    thread::sleep(Duration::from_secs(2));
    fixture.nudge(&receiver_pane, &receiver_prompt(receiver));
    fixture.wait_for_text(
        &receiver_pane,
        receiver.ready_marker(),
        RECEIVER_READY_TIMEOUT,
    );

    let sender_pane = fixture.spawn(sender);
    fixture.write_agents(&[(sender, &sender_pane), (receiver, &receiver_pane)]);
    thread::sleep(Duration::from_secs(2));
    fixture.nudge(&sender_pane, &sender_prompt(sender, receiver, body));

    let message = fixture.wait_for_message(body, SENDER_TIMEOUT);
    assert_eq!(message.from.as_str(), sender.actor());
    assert_eq!(message.to.as_str(), receiver.actor());

    let captured = fixture.wait_for_text(
        &receiver_pane,
        &format!("{}-FROM-{}", receiver.received_marker(), sender.actor()),
        RECEIVER_ACK_TIMEOUT,
    );
    assert!(captured.contains(body));

    let reply = fixture.wait_for_message(
        &format!("REPLY_RESPONSE:reply from {}", receiver.actor()),
        RECEIVER_ACK_TIMEOUT,
    );
    assert_eq!(reply.from.as_str(), receiver.actor());
    assert_eq!(reply.to.as_str(), sender.actor());
}

#[test]
#[ignore = "runs authenticated Codex and Claude harnesses through WezTerm"]
fn actual_codex_initiates_claude_receives_idle_message() {
    run_initiator_test(
        HarnessKind::Codex,
        HarnessKind::Claude,
        "REPLY_REQUEST:REAL-HARNESS-CODEX-TO-CLAUDE",
    );
}

#[test]
#[ignore = "runs authenticated Claude and Codex harnesses through WezTerm"]
fn actual_claude_initiates_codex_receives_idle_message() {
    run_initiator_test(
        HarnessKind::Claude,
        HarnessKind::Codex,
        "REPLY_REQUEST:REAL-HARNESS-CLAUDE-TO-CODEX",
    );
}
