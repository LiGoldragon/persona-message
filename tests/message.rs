use nota_codec::Error;
use persona_message::command::{CommandLine, Input};
use persona_message::delivery::PromptState;
use persona_message::resolver::{ActorIndex, ProcessAncestry};
use persona_message::router::SignalRouterFrameCodec;
use persona_message::schema::{
    Actor, ActorId, EndpointKind, EndpointTransport, Message, MessageIdKind,
};
use persona_message::store::{MessageStore, StorePath};
use signal_core::{AuthProof, FrameBody, Reply, Request, SemaVerb};
use signal_persona_message::{
    Frame, InboxEntry, InboxListing, MessageBody, MessageReply, MessageRequest, MessageSender,
    MessageSlot, SubmissionAcceptance,
};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::process::{Command, Stdio};

#[test]
fn message_round_trips_through_nota() {
    let text = "(Message m-1 thread-1 operator designer \"Need a design pass.\" [])";
    let message = Message::from_nota(text).expect("message decodes");

    assert_eq!(message.id.as_str(), "m-1");
    assert_eq!(message.thread.as_str(), "thread-1");
    assert_eq!(message.from.as_str(), "operator");
    assert_eq!(message.to.as_str(), "designer");

    let encoded = message.to_nota().expect("message encodes");
    assert_eq!(
        Message::from_nota(&encoded).expect("encoded decodes"),
        message
    );
}

#[test]
fn agents_config_resolves_process_ancestry() {
    let config = ActorIndex::from_actors(vec![
        Actor {
            name: ActorId::new("operator"),
            pid: 10,
            endpoint: None,
        },
        Actor {
            name: ActorId::new("designer"),
            pid: 20,
            endpoint: None,
        },
    ]);
    let ancestry = ProcessAncestry::from_pids(vec![40, 30, 20, 10]);

    let actor = config.resolve(&ancestry).expect("agent resolves");

    assert_eq!(actor.as_str(), "designer");
}

#[test]
fn actor_endpoint_round_trips_with_owned_endpoint() {
    let actor = Actor {
        name: ActorId::new("designer"),
        pid: 42,
        endpoint: Some(EndpointTransport {
            kind: EndpointKind::PtySocket,
            target: "/tmp/designer.sock".to_string(),
            aux: None,
        }),
    };

    let encoded = actor.to_nota().expect("actor encodes");
    let decoded = Actor::from_nota(&encoded).expect("actor decodes");

    assert!(encoded.contains("PtySocket"));
    assert_eq!(decoded, actor);
    assert_eq!(
        Actor::from_nota("(Actor operator 7 None)")
            .expect("actor decodes without endpoint")
            .endpoint,
        None
    );
    assert_eq!(
        Actor::from_nota(
            r#"(Actor responder 77 (EndpointTransport PtySocket "/tmp/responder.sock" None))"#
        )
        .expect("pty actor decodes")
        .endpoint
        .expect("endpoint exists")
        .target
        .as_str(),
        "/tmp/responder.sock"
    );
}

#[test]
fn actor_endpoint_kind_rejects_unknown_transport() {
    let err =
        Actor::from_nota(r#"(Actor responder 77 (EndpointTransport Bogus "/tmp/socket" None))"#)
            .expect_err("unknown endpoint transport is rejected");

    match err {
        Error::UnknownVariant { enum_name, got } => {
            assert_eq!(enum_name, "EndpointKind");
            assert_eq!(got, "Bogus");
        }
        other => panic!("expected UnknownVariant, got {other:?}"),
    }
}

#[test]
fn human_endpoint_does_not_inject_terminal_input() {
    let actor = Actor {
        name: ActorId::new("operator"),
        pid: std::process::id(),
        endpoint: Some(EndpointTransport {
            kind: EndpointKind::Human,
            target: "operator".to_string(),
            aux: None,
        }),
    };
    let prompt = persona_wezterm::terminal::TerminalPrompt::from_text(
        "(Message m-abc direct-designer-operator designer operator ready [])",
    );

    let delivered = actor.deliver(&prompt).expect("human endpoint is accepted");

    assert!(!delivered);
}

#[test]
fn prompt_state_reads_cursor_line_before_cursor() {
    let state = PromptState::from_cursor_line("> human draft", 13);

    assert_eq!(
        state,
        PromptState::Occupied {
            preview: "human draft".to_string()
        }
    );
}

#[test]
fn prompt_state_accepts_empty_prompt_prefixes() {
    assert_eq!(PromptState::from_cursor_line("> ", 2), PromptState::Empty);
    assert_eq!(PromptState::from_cursor_line("› ", 2), PromptState::Empty);
}

#[test]
fn store_filters_messages_by_recipient() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = MessageStore::from_path(StorePath::from_path(directory.path()));
    let operator_message =
        Message::from_nota("(Message m-1 thread-1 operator designer \"for designer\" [])")
            .expect("operator message decodes");
    let designer_message =
        Message::from_nota("(Message m-2 thread-1 designer operator \"for operator\" [])")
            .expect("designer message decodes");

    store.append(&operator_message).expect("operator append");
    store.append(&designer_message).expect("designer append");

    let designer_inbox = store
        .inbox(&ActorId::new("designer"))
        .expect("designer inbox reads");
    let operator_inbox = store
        .inbox(&ActorId::new("operator"))
        .expect("operator inbox reads");

    assert_eq!(designer_inbox, vec![operator_message]);
    assert_eq!(operator_inbox, vec![designer_message]);
}

#[test]
fn command_line_send_stamps_resolved_sender() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = MessageStore::from_path(StorePath::from_path(directory.path()));
    let actor = Actor {
        name: ActorId::new("operator"),
        pid: std::process::id(),
        endpoint: None,
    };
    std::fs::write(
        store.path().actor_index(),
        actor.to_nota().expect("actor encodes"),
    )
    .expect("actor index writes");
    let command = CommandLine::from_arguments([r#"(Send designer "typed hello")"#]);
    let mut output = Vec::new();

    command.run(&store, &mut output).expect("message sends");
    let messages = store.messages().expect("messages read");

    assert_eq!(messages.len(), 1);
    let id = messages[0].id.view().expect("message id has typed view");
    assert_eq!(id.kind(), MessageIdKind::Message);
    assert_eq!(id.short_hash().len(), 3);
    assert_eq!(messages[0].from.as_str(), "operator");
    assert_eq!(messages[0].to.as_str(), "designer");
    assert!(
        String::from_utf8(output)
            .expect("output is utf8")
            .contains("typed hello")
    );
}

#[test]
fn command_line_send_accepts_and_emits_bare_identifier_bodies() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = MessageStore::from_path(StorePath::from_path(directory.path()));
    let actor = Actor {
        name: ActorId::new("operator"),
        pid: std::process::id(),
        endpoint: None,
    };
    std::fs::write(
        store.path().actor_index(),
        actor.to_nota().expect("actor encodes"),
    )
    .expect("actor index writes");
    let command = CommandLine::from_arguments(["(Send designer ready-token)"]);
    let mut output = Vec::new();

    command.run(&store, &mut output).expect("message sends");
    let ledger = std::fs::read_to_string(store.path().message_log()).expect("ledger reads");

    assert!(ledger.contains(" ready-token []"));
    assert!(!ledger.contains("\"ready-token\""));
}

#[test]
fn command_line_send_can_route_through_persona_router() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store_path = directory.path().join("store");
    let router_socket = directory.path().join("router.sock");
    std::fs::create_dir_all(&store_path).expect("store directory");
    let listener = UnixListener::bind(&router_socket).expect("router socket binds");
    let message = env!("CARGO_BIN_EXE_message");
    let start = directory.path().join("start");
    let shell = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "while [ ! -f '{}' ]; do sleep 0.05; done; '{}' '(Send designer router-hello)'",
            start.display(),
            message
        ))
        .env("PERSONA_MESSAGE_STORE", &store_path)
        .env("PERSONA_ROUTER_SOCKET", &router_socket)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("message shell starts");
    let actor = Actor {
        name: ActorId::new("operator"),
        pid: shell.id(),
        endpoint: None,
    };
    std::fs::write(
        store_path.join("actors.nota"),
        actor.to_nota().expect("actor encodes"),
    )
    .expect("actor index writes");

    let router = std::thread::spawn(move || {
        std::fs::write(&start, "").expect("start marker writes");
        let (mut stream, _) = listener.accept().expect("router accepts");
        let mut line = String::new();
        BufReader::new(stream.try_clone().expect("stream clones"))
            .read_line(&mut line)
            .expect("router input reads");
        writeln!(stream, "(DeliveryChanged 1 0)").expect("router response writes");
        line
    });

    let output = shell.wait_with_output().expect("message shell exits");
    let route = router.join().expect("router thread joins");
    let store = MessageStore::from_path(StorePath::from_path(&store_path));
    let messages = store.messages().expect("messages read");

    assert!(output.status.success());
    assert!(route.starts_with("(RouteMessage (Message m-"));
    assert!(route.contains(" direct-operator-designer operator designer router-hello []"));
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].from.as_str(), "operator");
    assert_eq!(messages[0].to.as_str(), "designer");
    assert_eq!(messages[0].body.as_str(), "router-hello");
}

#[test]
fn command_line_send_routes_signal_frame_without_writing_local_ledger() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store_path = directory.path().join("store");
    let router_socket = directory.path().join("router.signal.sock");
    std::fs::create_dir_all(&store_path).expect("store directory");
    let listener = UnixListener::bind(&router_socket).expect("router socket binds");
    let message = env!("CARGO_BIN_EXE_message");
    let start = directory.path().join("start");
    let shell = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "while [ ! -f '{}' ]; do sleep 0.05; done; '{}' '(Send designer signal-hello)'",
            start.display(),
            message
        ))
        .env("PERSONA_MESSAGE_STORE", &store_path)
        .env("PERSONA_MESSAGE_ROUTER_SOCKET", &router_socket)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("message shell starts");
    let actor = Actor {
        name: ActorId::new("operator"),
        pid: shell.id(),
        endpoint: None,
    };
    std::fs::write(
        store_path.join("actors.nota"),
        actor.to_nota().expect("actor encodes"),
    )
    .expect("actor index writes");

    let router = std::thread::spawn(move || {
        std::fs::write(&start, "").expect("start marker writes");
        let (mut stream, _) = listener.accept().expect("router accepts");
        let codec = SignalRouterFrameCodec::default();
        let frame = codec.read_frame(&mut stream).expect("router input reads");
        let Some(AuthProof::LocalOperator(proof)) = frame.auth() else {
            panic!("expected local operator auth proof");
        };
        assert_eq!(proof.operator(), "operator");
        let FrameBody::Request(Request::Operation { verb, payload }) = frame.into_body() else {
            panic!("expected signal request frame");
        };
        assert_eq!(verb, SemaVerb::Assert);
        let MessageRequest::MessageSubmission(submission) = payload else {
            panic!("expected message submission");
        };
        assert_eq!(submission.recipient.as_str(), "designer");
        assert_eq!(submission.body.as_str(), "signal-hello");
        let reply = Frame::new(FrameBody::Reply(Reply::operation(
            MessageReply::SubmissionAccepted(SubmissionAcceptance {
                message_slot: MessageSlot::new(7),
            }),
        )));
        codec
            .write_frame(&mut stream, &reply)
            .expect("router reply writes");
        submission
    });

    let output = shell.wait_with_output().expect("message shell exits");
    let submission = router.join().expect("router thread joins");
    let store = MessageStore::from_path(StorePath::from_path(&store_path));
    let messages = store.messages().expect("messages read");
    let text = String::from_utf8(output.stdout).expect("output is utf8");

    assert!(output.status.success());
    assert_eq!(submission.body, MessageBody::new("signal-hello"));
    assert!(text.contains("(SubmissionAccepted 7)"));
    assert!(messages.is_empty());
    assert!(
        !store.path().message_log().exists(),
        "signal router path must not create local messages.nota.log"
    );
}

#[test]
fn command_line_inbox_routes_signal_frame_without_reading_local_ledger() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store_path = directory.path().join("store");
    let router_socket = directory.path().join("router.signal.sock");
    std::fs::create_dir_all(&store_path).expect("store directory");
    std::fs::write(
        store_path.join("messages.nota.log"),
        "(Message m-old direct-operator-designer operator designer stale-local [])\n",
    )
    .expect("stale local ledger writes");
    let listener = UnixListener::bind(&router_socket).expect("router socket binds");
    let message = env!("CARGO_BIN_EXE_message");
    let start = directory.path().join("start");
    let shell = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "while [ ! -f '{}' ]; do sleep 0.05; done; '{}' '(Inbox designer)'",
            start.display(),
            message
        ))
        .env("PERSONA_MESSAGE_STORE", &store_path)
        .env("PERSONA_MESSAGE_ROUTER_SOCKET", &router_socket)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("message shell starts");
    let actor = Actor {
        name: ActorId::new("operator"),
        pid: shell.id(),
        endpoint: None,
    };
    std::fs::write(
        store_path.join("actors.nota"),
        actor.to_nota().expect("actor encodes"),
    )
    .expect("actor index writes");

    let router = std::thread::spawn(move || {
        std::fs::write(&start, "").expect("start marker writes");
        let (mut stream, _) = listener.accept().expect("router accepts");
        let codec = SignalRouterFrameCodec::default();
        let frame = codec.read_frame(&mut stream).expect("router input reads");
        let Some(AuthProof::LocalOperator(proof)) = frame.auth() else {
            panic!("expected local operator auth proof");
        };
        assert_eq!(proof.operator(), "operator");
        let FrameBody::Request(Request::Operation { verb, payload }) = frame.into_body() else {
            panic!("expected signal request frame");
        };
        assert_eq!(verb, SemaVerb::Assert);
        let MessageRequest::InboxQuery(query) = payload else {
            panic!("expected inbox query");
        };
        assert_eq!(query.recipient.as_str(), "designer");
        let reply = Frame::new(FrameBody::Reply(Reply::operation(
            MessageReply::InboxListing(InboxListing {
                messages: vec![InboxEntry {
                    message_slot: MessageSlot::new(3),
                    sender: MessageSender::new("operator"),
                    body: MessageBody::new("router-only"),
                }],
            }),
        )));
        codec
            .write_frame(&mut stream, &reply)
            .expect("router reply writes");
    });

    let output = shell.wait_with_output().expect("message shell exits");
    router.join().expect("router thread joins");
    let text = String::from_utf8(output.stdout).expect("output is utf8");

    assert!(output.status.success());
    assert!(text.contains("RouterInboxListing"));
    assert!(text.contains("router-only"));
    assert!(!text.contains("stale-local"));
}

#[test]
fn command_line_registers_actor_for_current_session() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = MessageStore::from_path(StorePath::from_path(directory.path()));
    let command = CommandLine::from_arguments(["(Register operator None)"]);
    let mut output = Vec::new();

    command.run(&store, &mut output).expect("actor registers");

    let actors = store.actors().expect("actors read");
    let actor = actors
        .actor(&ActorId::new("operator"))
        .expect("registered actor exists");
    assert!(actor.pid > 0);
    assert_eq!(actor.endpoint, None);
    assert!(
        String::from_utf8(output)
            .expect("output is utf8")
            .contains("(Registered (Actor operator")
    );
}

#[test]
fn command_line_agents_lists_registered_actors() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = MessageStore::from_path(StorePath::from_path(directory.path()));
    store
        .register(&Actor {
            name: ActorId::new("operator"),
            pid: 10,
            endpoint: None,
        })
        .expect("operator registers");
    store
        .register(&Actor {
            name: ActorId::new("designer"),
            pid: 20,
            endpoint: None,
        })
        .expect("designer registers");
    let command = CommandLine::from_arguments(["(Agents)"]);
    let mut output = Vec::new();

    command.run(&store, &mut output).expect("agents list");
    let text = String::from_utf8(output).expect("output is utf8");

    assert!(text.contains("(KnownActors ["));
    assert!(text.contains("(Actor operator 10 None)"));
    assert!(text.contains("(Actor designer 20 None)"));
}

#[test]
fn register_replaces_existing_actor_endpoint() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = MessageStore::from_path(StorePath::from_path(directory.path()));
    store
        .register(&Actor {
            name: ActorId::new("operator"),
            pid: 10,
            endpoint: None,
        })
        .expect("operator registers first");
    let replacement = Actor {
        name: ActorId::new("operator"),
        pid: 11,
        endpoint: Some(EndpointTransport {
            kind: EndpointKind::Human,
            target: "operator".to_string(),
            aux: None,
        }),
    };

    store.register(&replacement).expect("operator replaces");
    let actors = store.actors().expect("actors read");

    assert_eq!(actors.actors().len(), 1);
    assert_eq!(actors.actor(&ActorId::new("operator")), Some(&replacement));
}

#[test]
fn command_line_takes_exactly_one_argument() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = MessageStore::from_path(StorePath::from_path(directory.path()));
    let command = CommandLine::from_arguments(["(Inbox", "designer)"]);
    let mut output = Vec::new();

    let error = command
        .run(&store, &mut output)
        .expect_err("split nota is rejected");

    assert!(
        error
            .to_string()
            .contains("unexpected command-line argument")
    );
}

#[test]
fn input_rejects_unknown_record_heads() {
    let error = Input::from_nota("(Bead message operator designer \"legacy\")")
        .expect_err("bead is not persona message input");

    assert!(error.to_string().contains("Bead"));
}
