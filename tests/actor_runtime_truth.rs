use std::fs;
use std::path::{Path, PathBuf};

use persona_message::command::{Agents, Input, Output};
use persona_message::daemon::{
    ActorRequestCountProbe, DaemonEnvelope, DaemonRequest, MessageDaemonActorHandle,
};
use persona_message::schema::{Actor, ActorId};
use persona_message::store::{MessageStore, StorePath};

struct SourceFile {
    path: PathBuf,
    content: String,
}

impl SourceFile {
    fn read(path: PathBuf) -> Self {
        let content = fs::read_to_string(&path).expect("source file is readable");
        Self { path, content }
    }

    fn is_guard_source(&self) -> bool {
        self.path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "actor_runtime_truth.rs")
    }

    fn contains(&self, fragment: &str) -> bool {
        self.content.contains(fragment)
    }
}

struct SourceTree {
    root: PathBuf,
}

impl SourceTree {
    fn new() -> Self {
        Self {
            root: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        }
    }

    fn guarded_files(&self) -> Vec<SourceFile> {
        let mut files = vec![self.root.join("Cargo.toml"), self.root.join("Cargo.lock")];
        files.extend(self.source_files());
        files.extend(self.test_files());
        files.into_iter().map(SourceFile::read).collect()
    }

    fn source_files(&self) -> Vec<PathBuf> {
        let src = self.root.join("src");
        fs::read_dir(src)
            .expect("source directory is readable")
            .map(|entry| entry.expect("source entry is readable").path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
            .collect()
    }

    fn test_files(&self) -> Vec<PathBuf> {
        let tests = self.root.join("tests");
        fs::read_dir(tests)
            .expect("tests directory is readable")
            .map(|entry| entry.expect("test entry is readable").path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
            .collect()
    }
}

#[test]
fn message_daemon_actor_cannot_use_non_kameo_runtime() {
    let forbidden_fragments = [
        "ractor =",
        "name = \"ractor\"",
        "use ractor",
        "ractor::",
        "RpcReplyPort",
        "ActorProcessingErr",
    ];

    let mut violations = Vec::new();
    for file in SourceTree::new().guarded_files() {
        if file.is_guard_source() {
            continue;
        }
        for fragment in forbidden_fragments {
            if file.contains(fragment) {
                violations.push(format!("{} contains {fragment}", file.path.display()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "non-kameo daemon actor runtime violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn message_daemon_cannot_bypass_actor_mailbox() {
    let source = SourceFile::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("daemon.rs"),
    );

    assert!(source.contains("MessageDaemonActorHandle::start"));
    assert!(source.contains("actor.execute(envelope)"));
    assert!(!source.contains("let response = envelope.execute(&self.store)"));
}

#[test]
fn message_daemon_actor_cannot_be_empty_marker() {
    let daemon_source = SourceFile::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("daemon.rs"),
    );
    let store_source = SourceFile::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("actors")
            .join("message_store.rs"),
    );

    assert!(daemon_source.contains("pub struct MessageDaemonActor {"));
    assert!(daemon_source.contains("store_actor: ActorRef<MessageStoreActor>,"));
    assert!(daemon_source.contains("executed_request_count: u64,"));
    assert!(daemon_source.contains("emitted_response_count: u64,"));
    assert!(store_source.contains("pub struct MessageStoreActor {"));
    assert!(store_source.contains("store: MessageStore,"));
    assert!(store_source.contains("executed_request_count: u64,"));
}

#[test]
fn message_store_mutation_cannot_skip_child_actor() {
    let source = SourceFile::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("daemon.rs"),
    );

    assert!(source.contains("MessageStoreActor::supervise(&actor_reference, store)"));
    assert!(source.contains(".ask(ExecuteStoreEnvelope"));
    assert!(!source.contains("message.envelope.execute(&self.store)"));
}

#[test]
fn message_actor_messages_cannot_be_empty_markers() {
    let forbidden_empty_markers = [
        "pub struct ReadDaemonActorRequestCount;",
        "pub struct ReadStoreRequestCount;",
        "pub struct ReadStoreActorRequestCount;",
        "struct ReadDaemonActorRequestCount;",
        "struct ReadStoreRequestCount;",
        "struct ReadStoreActorRequestCount;",
    ];

    let mut violations = Vec::new();
    for file in SourceTree::new().guarded_files() {
        if file.is_guard_source() {
            continue;
        }
        for marker in forbidden_empty_markers {
            if file.contains(marker) {
                violations.push(format!("{} contains {marker}", file.path.display()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "empty actor-message marker violations:\n{}",
        violations.join("\n")
    );
}

#[tokio::test]
async fn message_daemon_actor_cannot_skip_known_actor_state() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = MessageStore::from_path(StorePath::from_path(directory.path()));
    let actor = Actor {
        name: ActorId::new("operator"),
        pid: std::process::id(),
        endpoint: None,
    };
    std::fs::create_dir_all(store.path().root()).expect("store root writes");
    std::fs::write(
        store.path().actor_index(),
        format!("{}\n", actor.to_nota().expect("actor encodes")),
    )
    .expect("actor index writes");
    let daemon = MessageDaemonActorHandle::start(store).await;

    let response = daemon
        .execute(DaemonEnvelope::Request(DaemonRequest::from_input(
            std::process::id(),
            Input::Agents(Agents {}),
        )))
        .await
        .expect("daemon actor executes request");

    match response {
        DaemonEnvelope::Response(Output::KnownActors(output)) => {
            assert_eq!(output.actors.len(), 1);
            assert_eq!(output.actors[0].name.as_str(), "operator");
        }
        other => panic!("expected known actors response, got {other:?}"),
    }
    let daemon_count = daemon
        .executed_request_count(ActorRequestCountProbe::expecting_at_least(1))
        .await
        .expect("daemon actor count reads through typed message");
    let store_count = daemon
        .store_executed_request_count(ActorRequestCountProbe::expecting_at_least(1))
        .await
        .expect("store actor count reads through typed message");

    assert_eq!(daemon_count.observed(), 1);
    assert_eq!(store_count.observed(), 1);
    assert!(daemon_count.satisfied());
    assert!(store_count.satisfied());

    daemon.stop().await.expect("daemon actor stops");
}
