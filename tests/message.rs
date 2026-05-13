use nota_codec::Error;
use persona_message::command::{CommandLine, Input};
use persona_message::router::SignalRouterFrameCodec;
use signal_core::{FrameBody, Reply, Request, SemaVerb};
use signal_persona_message::{
    Frame, InboxEntry, InboxListing, MessageBody, MessageKind, MessageReply, MessageRequest,
    MessageSender, MessageSlot, SubmissionAcceptance,
};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

struct MessageFixture {
    directory: tempfile::TempDir,
}

impl MessageFixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("temporary directory");
        Self { directory }
    }

    fn router_socket_path(&self) -> PathBuf {
        self.directory.path().join("router.signal.sock")
    }

    fn message_socket_path(&self) -> PathBuf {
        self.directory.path().join("message.signal.sock")
    }

    fn start_path(&self) -> PathBuf {
        self.directory.path().join("start")
    }

    fn local_ledger_path(&self) -> PathBuf {
        self.directory
            .path()
            .join(".persona-message")
            .join(["messages", ".nota.log"].concat())
    }

    fn spawn_message_after_start(
        &self,
        start_path: &Path,
        message_socket_path: Option<&Path>,
        input: &str,
    ) -> std::process::Child {
        let mut command = Command::new("sh");
        command.arg("-c").arg(format!(
            "while [ ! -f '{}' ]; do sleep 0.05; done; '{}' '{}'",
            start_path.display(),
            env!("CARGO_BIN_EXE_message"),
            input
        ));
        command.current_dir(self.directory.path());
        if let Some(message_socket_path) = message_socket_path {
            command.env("PERSONA_MESSAGE_SOCKET", message_socket_path);
        }
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        command.spawn().expect("message shell starts")
    }

    fn spawn_daemon_after_router_start(
        &self,
        start_path: &Path,
        message_socket_path: &Path,
        router_socket_path: &Path,
    ) -> std::process::Child {
        let mut command = Command::new("sh");
        command.arg("-c").arg(format!(
            "while [ ! -f '{}' ]; do sleep 0.05; done; '{}' '{}' '{}'",
            start_path.display(),
            env!("CARGO_BIN_EXE_persona-message-daemon"),
            message_socket_path.display(),
            router_socket_path.display()
        ));
        command.current_dir(self.directory.path());
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        command.spawn().expect("message daemon shell starts")
    }
}

struct RouterReply {
    frame: Frame,
}

impl RouterReply {
    fn accepted(slot: u64) -> Self {
        Self {
            frame: Frame::new(FrameBody::Reply(Reply::operation(
                MessageReply::SubmissionAccepted(SubmissionAcceptance {
                    message_slot: MessageSlot::new(slot),
                }),
            ))),
        }
    }

    fn inbox(sender: &str, body: &str) -> Self {
        Self {
            frame: Frame::new(FrameBody::Reply(Reply::operation(
                MessageReply::InboxListing(InboxListing {
                    messages: vec![InboxEntry {
                        message_slot: MessageSlot::new(3),
                        sender: MessageSender::new(sender),
                        body: MessageBody::new(body),
                    }],
                }),
            ))),
        }
    }
}

struct RecordedFrame {
    request: MessageRequest,
}

struct FakeRouter {
    listener: UnixListener,
    start_path: PathBuf,
}

impl FakeRouter {
    fn bind(socket_path: &Path, start_path: PathBuf) -> Self {
        Self {
            listener: UnixListener::bind(socket_path).expect("router socket binds"),
            start_path,
        }
    }

    fn serve(self, reply: RouterReply) -> std::thread::JoinHandle<RecordedFrame> {
        std::thread::spawn(move || {
            std::fs::write(&self.start_path, "").expect("start marker writes");
            let (mut stream, _) = self.listener.accept().expect("router accepts");
            let codec = SignalRouterFrameCodec::default();
            let frame = codec.read_frame(&mut stream).expect("router input reads");
            let FrameBody::Request(Request::Operation { verb, payload }) = frame.into_body() else {
                panic!("expected signal request frame");
            };
            assert_eq!(verb, SemaVerb::Assert);
            codec
                .write_frame(&mut stream, &reply.frame)
                .expect("router reply writes");
            RecordedFrame { request: payload }
        })
    }
}

#[test]
fn command_line_send_routes_signal_frame_without_writing_local_ledger() {
    let fixture = MessageFixture::new();
    let message_socket_path = fixture.message_socket_path();
    let start_path = fixture.start_path();
    let fake_router =
        FakeRouter::bind(&message_socket_path, start_path.clone()).serve(RouterReply::accepted(7));
    let shell = fixture.spawn_message_after_start(
        &start_path,
        Some(&message_socket_path),
        "(Send designer signal-hello)",
    );

    let output = shell.wait_with_output().expect("message shell exits");
    let recorded = fake_router.join().expect("router thread joins");
    let text = String::from_utf8(output.stdout).expect("output is utf8");

    assert!(output.status.success());
    let MessageRequest::MessageSubmission(submission) = recorded.request else {
        panic!("expected message submission");
    };
    assert_eq!(submission.recipient.as_str(), "designer");
    assert_eq!(submission.kind, MessageKind::Send);
    assert_eq!(submission.body, MessageBody::new("signal-hello"));
    assert!(text.contains("(SubmissionAccepted 7)"));
    assert!(!fixture.local_ledger_path().exists());
}

#[test]
fn command_line_send_preserves_bare_identifier_body_in_signal_payload() {
    let fixture = MessageFixture::new();
    let message_socket_path = fixture.message_socket_path();
    let start_path = fixture.start_path();
    let fake_router =
        FakeRouter::bind(&message_socket_path, start_path.clone()).serve(RouterReply::accepted(8));
    let shell = fixture.spawn_message_after_start(
        &start_path,
        Some(&message_socket_path),
        "(Send designer ready-token)",
    );

    let output = shell.wait_with_output().expect("message shell exits");
    let recorded = fake_router.join().expect("router thread joins");

    assert!(output.status.success());
    let MessageRequest::MessageSubmission(submission) = recorded.request else {
        panic!("expected message submission");
    };
    assert_eq!(submission.kind, MessageKind::Send);
    assert_eq!(submission.body.as_str(), "ready-token");
}

#[test]
fn command_line_inbox_routes_signal_frame_without_reading_local_ledger() {
    let fixture = MessageFixture::new();
    let message_socket_path = fixture.message_socket_path();
    let start_path = fixture.start_path();
    let local_ledger_path = fixture.local_ledger_path();
    std::fs::create_dir_all(local_ledger_path.parent().expect("ledger parent"))
        .expect("ledger directory writes");
    std::fs::write(
        &local_ledger_path,
        "(Message m-old direct-operator-designer operator designer stale-local [])\n",
    )
    .expect("stale local ledger writes");
    let fake_router = FakeRouter::bind(&message_socket_path, start_path.clone())
        .serve(RouterReply::inbox("operator", "router-only"));
    let shell = fixture.spawn_message_after_start(
        &start_path,
        Some(&message_socket_path),
        "(Inbox designer)",
    );

    let output = shell.wait_with_output().expect("message shell exits");
    let recorded = fake_router.join().expect("router thread joins");
    let text = String::from_utf8(output.stdout).expect("output is utf8");

    assert!(output.status.success());
    let MessageRequest::InboxQuery(query) = recorded.request else {
        panic!("expected inbox query");
    };
    assert_eq!(query.recipient.as_str(), "designer");
    assert!(text.contains("RouterInboxListing"));
    assert!(text.contains("router-only"));
    assert!(!text.contains("stale-local"));
}

#[test]
fn command_line_send_requires_message_socket() {
    let fixture = MessageFixture::new();
    let start_path = fixture.start_path();
    let shell = fixture.spawn_message_after_start(&start_path, None, "(Send designer hello)");
    std::fs::write(&start_path, "").expect("start marker writes");

    let output = shell.wait_with_output().expect("message shell exits");
    let stderr = String::from_utf8(output.stderr).expect("stderr is utf8");

    assert!(!output.status.success());
    assert!(stderr.contains("SignalMessageSocketMissing"));
    assert!(!fixture.local_ledger_path().exists());
}

#[test]
fn persona_message_daemon_forwards_cli_signal_frame_to_router_socket() {
    let fixture = MessageFixture::new();
    let message_socket_path = fixture.message_socket_path();
    let router_socket_path = fixture.router_socket_path();
    let start_path = fixture.start_path();
    let fake_router =
        FakeRouter::bind(&router_socket_path, start_path.clone()).serve(RouterReply::accepted(11));
    let mut daemon = fixture.spawn_daemon_after_router_start(
        &start_path,
        &message_socket_path,
        &router_socket_path,
    );

    for _ in 0..100 {
        if message_socket_path.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(message_socket_path.exists(), "daemon bound message socket");
    let shell = fixture.spawn_message_after_start(
        &start_path,
        Some(&message_socket_path),
        "(Send designer daemon-forward)",
    );

    let output = shell.wait_with_output().expect("message shell exits");
    let recorded = fake_router.join().expect("router thread joins");
    let text = String::from_utf8(output.stdout).expect("output is utf8");
    let _ = daemon.kill();
    let _ = daemon.wait();

    assert!(output.status.success());
    let MessageRequest::StampedMessageSubmission(stamped) = recorded.request else {
        panic!("expected daemon-forwarded stamped message submission");
    };
    let submission = stamped.submission;
    assert_eq!(submission.recipient.as_str(), "designer");
    assert_eq!(submission.kind, MessageKind::Send);
    assert_eq!(submission.body.as_str(), "daemon-forward");
    assert!(stamped.stamped_at.into_u64() > 0);
    assert!(text.contains("(SubmissionAccepted 11)"));
    assert!(!fixture.local_ledger_path().exists());
}

#[test]
fn command_line_takes_exactly_one_argument() {
    let command = CommandLine::from_arguments(["(Inbox", "designer)"]);
    let mut output = Vec::new();

    let error = command
        .run(&mut output)
        .expect_err("split nota is rejected");

    assert!(
        error
            .to_string()
            .contains("unexpected command-line argument")
    );
}

#[test]
fn input_rejects_unknown_record_heads() {
    let error = Input::from_nota("(Bead)").expect_err("unknown input is rejected");

    match error {
        persona_message::Error::Nota(Error::UnknownKindForVerb { verb, got }) => {
            assert_eq!(verb, "Input");
            assert_eq!(got, "Bead");
        }
        other => panic!("expected unknown input kind, got {other:?}"),
    }
}
