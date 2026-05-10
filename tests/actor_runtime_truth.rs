use std::fs;
use std::path::{Path, PathBuf};

use persona_message::command::{Agents, Input, Output};
use persona_message::daemon::{DaemonEnvelope, DaemonRequest, MessageDaemonActorHandle};
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
    let source = SourceFile::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("daemon.rs"),
    );

    assert!(source.contains("pub struct MessageDaemonActor {"));
    assert!(source.contains("store: MessageStore,"));
    assert!(source.contains("executed_request_count: u64,"));
    assert!(source.contains("emitted_response_count: u64,"));
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
    daemon.stop().await.expect("daemon actor stops");
}
