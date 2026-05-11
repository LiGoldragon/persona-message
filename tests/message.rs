use nota_codec::Error;
use persona_message::command::{CommandLine, Input};
use persona_message::resolver::{ActorIndex, ActorIndexPath, ProcessAncestry};
use persona_message::router::SignalRouterFrameCodec;
use persona_message::schema::{Actor, ActorId};
use signal_core::{AuthProof, FrameBody, Reply, Request, SemaVerb};
use signal_persona_message::{
    Frame, InboxEntry, InboxListing, MessageBody, MessageReply, MessageRequest, MessageSender,
    MessageSlot, SubmissionAcceptance,
};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

struct ActorIndexFixture {
    directory: tempfile::TempDir,
    store_path: PathBuf,
}

impl ActorIndexFixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store_path = directory.path().join("store");
        std::fs::create_dir_all(&store_path).expect("store directory");
        Self {
            directory,
            store_path,
        }
    }

    fn store_path(&self) -> &Path {
        self.store_path.as_path()
    }

    fn actor_index_path(&self) -> ActorIndexPath {
        ActorIndexPath::from_path(self.store_path.join("actors.nota"))
    }

    fn router_socket_path(&self) -> PathBuf {
        self.directory.path().join("router.signal.sock")
    }

    fn start_path(&self) -> PathBuf {
        self.directory.path().join("start")
    }

    fn write_actor(&self, name: &str, pid: u32) {
        let actor = Actor {
            name: ActorId::new(name),
            pid,
        };
        std::fs::write(
            self.store_path.join("actors.nota"),
            format!("{}\n", actor.to_nota().expect("actor encodes")),
        )
        .expect("actor index writes");
    }

    fn spawn_message_after_start(
        &self,
        start_path: &Path,
        router_socket_path: Option<&Path>,
        input: &str,
    ) -> std::process::Child {
        let mut command = Command::new("sh");
        command.arg("-c").arg(format!(
            "while [ ! -f '{}' ]; do sleep 0.05; done; '{}' '{}'",
            start_path.display(),
            env!("CARGO_BIN_EXE_message"),
            input
        ));
        command.env("PERSONA_MESSAGE_STORE", &self.store_path);
        if let Some(router_socket_path) = router_socket_path {
            command.env("PERSONA_MESSAGE_ROUTER_SOCKET", router_socket_path);
        }
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        command.spawn().expect("message shell starts")
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
    auth_actor: String,
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
            let Some(AuthProof::LocalOperator(proof)) = frame.auth() else {
                panic!("expected local operator auth proof");
            };
            let auth_actor = proof.operator().to_string();
            let FrameBody::Request(Request::Operation { verb, payload }) = frame.into_body() else {
                panic!("expected signal request frame");
            };
            assert_eq!(verb, SemaVerb::Assert);
            codec
                .write_frame(&mut stream, &reply.frame)
                .expect("router reply writes");
            RecordedFrame {
                auth_actor,
                request: payload,
            }
        })
    }
}

#[test]
fn actor_index_resolves_process_ancestry() {
    let config = ActorIndex::from_actors(vec![
        Actor {
            name: ActorId::new("operator"),
            pid: 10,
        },
        Actor {
            name: ActorId::new("designer"),
            pid: 20,
        },
    ]);
    let ancestry = ProcessAncestry::from_pids(vec![40, 30, 20, 10]);

    let actor = config.resolve(&ancestry).expect("agent resolves");

    assert_eq!(actor.as_str(), "designer");
}

#[test]
fn actor_index_file_round_trips_without_endpoint_vocabulary() {
    let actor = Actor {
        name: ActorId::new("operator"),
        pid: 42,
    };

    let encoded = actor.to_nota().expect("actor encodes");
    let decoded = Actor::from_nota(&encoded).expect("actor decodes");

    assert_eq!(encoded, "(Actor operator 42)");
    assert_eq!(decoded, actor);
}

#[test]
fn actor_index_path_reads_legacy_store_environment_directory() {
    let fixture = ActorIndexFixture::new();
    fixture.write_actor("operator", std::process::id());

    let resolved = fixture
        .actor_index_path()
        .resolve_current_process()
        .expect("actor resolves");

    assert_eq!(resolved.as_str(), "operator");
}

#[test]
fn command_line_send_routes_signal_frame_without_writing_local_ledger() {
    let fixture = ActorIndexFixture::new();
    let router_socket_path = fixture.router_socket_path();
    let start_path = fixture.start_path();
    let fake_router =
        FakeRouter::bind(&router_socket_path, start_path.clone()).serve(RouterReply::accepted(7));
    let shell = fixture.spawn_message_after_start(
        &start_path,
        Some(&router_socket_path),
        "(Send designer signal-hello)",
    );
    fixture.write_actor("operator", shell.id());

    let output = shell.wait_with_output().expect("message shell exits");
    let recorded = fake_router.join().expect("router thread joins");
    let text = String::from_utf8(output.stdout).expect("output is utf8");

    assert!(output.status.success());
    assert_eq!(recorded.auth_actor, "operator");
    let MessageRequest::MessageSubmission(submission) = recorded.request else {
        panic!("expected message submission");
    };
    assert_eq!(submission.recipient.as_str(), "designer");
    assert_eq!(submission.body, MessageBody::new("signal-hello"));
    assert!(text.contains("(SubmissionAccepted 7)"));
    assert!(
        !fixture
            .store_path()
            .join(["messages", ".nota.log"].concat())
            .exists(),
        "signal router path must not create the retired local ledger"
    );
}

#[test]
fn command_line_send_preserves_bare_identifier_body_in_signal_payload() {
    let fixture = ActorIndexFixture::new();
    let router_socket_path = fixture.router_socket_path();
    let start_path = fixture.start_path();
    let fake_router =
        FakeRouter::bind(&router_socket_path, start_path.clone()).serve(RouterReply::accepted(8));
    let shell = fixture.spawn_message_after_start(
        &start_path,
        Some(&router_socket_path),
        "(Send designer ready-token)",
    );
    fixture.write_actor("operator", shell.id());

    let output = shell.wait_with_output().expect("message shell exits");
    let recorded = fake_router.join().expect("router thread joins");

    assert!(output.status.success());
    let MessageRequest::MessageSubmission(submission) = recorded.request else {
        panic!("expected message submission");
    };
    assert_eq!(submission.body.as_str(), "ready-token");
}

#[test]
fn command_line_inbox_routes_signal_frame_without_reading_local_ledger() {
    let fixture = ActorIndexFixture::new();
    let router_socket_path = fixture.router_socket_path();
    let start_path = fixture.start_path();
    std::fs::write(
        fixture
            .store_path()
            .join(["messages", ".nota.log"].concat()),
        "(Message m-old direct-operator-designer operator designer stale-local [])\n",
    )
    .expect("stale local ledger writes");
    let fake_router = FakeRouter::bind(&router_socket_path, start_path.clone())
        .serve(RouterReply::inbox("operator", "router-only"));
    let shell = fixture.spawn_message_after_start(
        &start_path,
        Some(&router_socket_path),
        "(Inbox designer)",
    );
    fixture.write_actor("operator", shell.id());

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
fn command_line_send_requires_router_socket() {
    let fixture = ActorIndexFixture::new();
    let start_path = fixture.start_path();
    let shell = fixture.spawn_message_after_start(&start_path, None, "(Send designer hello)");
    fixture.write_actor("operator", shell.id());
    std::fs::write(&start_path, "").expect("start marker writes");

    let output = shell.wait_with_output().expect("message shell exits");
    let stderr = String::from_utf8(output.stderr).expect("stderr is utf8");

    assert!(!output.status.success());
    assert!(stderr.contains("SignalRouterSocketMissing"));
    assert!(
        !fixture
            .store_path()
            .join(["messages", ".nota.log"].concat())
            .exists()
    );
}

#[test]
fn command_line_takes_exactly_one_argument() {
    let actor_index_path = ActorIndexPath::from_path("/tmp/nonexistent-actors.nota");
    let command = CommandLine::from_arguments(["(Inbox", "designer)"]);
    let mut output = Vec::new();

    let error = command
        .run(&actor_index_path, &mut output)
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
