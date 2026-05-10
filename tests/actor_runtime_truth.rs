use std::fs;
use std::path::{Path, PathBuf};

use persona_message::command::{Agents, Input, Output};
use persona_message::daemon::{
    DaemonEnvelope, DaemonRequest, DaemonRoot, ExecuteEnvelope, ReadLedgerRequestCount,
    ReadRootRequestCount, RequestCountProbe,
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
        self.rust_files_below(self.root.join("src"))
    }

    fn test_files(&self) -> Vec<PathBuf> {
        self.rust_files_below(self.root.join("tests"))
    }

    fn rust_files_below(&self, root: PathBuf) -> Vec<PathBuf> {
        let mut pending = vec![root];
        let mut files = Vec::new();
        while let Some(path) = pending.pop() {
            for entry in fs::read_dir(path).expect("source directory is readable") {
                let path = entry.expect("source entry is readable").path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().is_some_and(|extension| extension == "rs") {
                    files.push(path);
                }
            }
        }
        files
    }
}

#[test]
fn daemon_runtime_cannot_use_non_kameo_runtime() {
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
        "non-kameo daemon runtime violations:\n{}",
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

    assert!(source.contains("DaemonRoot::start"));
    assert!(source.contains("actor.ask(ExecuteEnvelope"));
    assert!(!source.contains("let response = envelope.execute(&self.store)"));
}

#[test]
fn daemon_and_ledger_cannot_be_empty_markers() {
    let daemon_source = SourceFile::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("daemon.rs"),
    );
    let ledger_source = SourceFile::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("actors")
            .join("ledger.rs"),
    );

    assert!(daemon_source.contains("pub struct DaemonRoot {"));
    assert!(daemon_source.contains("ledger: ActorRef<ledger::Ledger>,"));
    assert!(daemon_source.contains("executed_request_count: u64,"));
    assert!(daemon_source.contains("emitted_response_count: u64,"));
    assert!(ledger_source.contains("pub struct Ledger {"));
    assert!(ledger_source.contains("store: MessageStore,"));
    assert!(ledger_source.contains("executed_request_count: u64,"));
}

#[test]
fn message_store_mutation_cannot_skip_ledger() {
    let source = SourceFile::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("daemon.rs"),
    );

    assert!(source.contains("ledger::Ledger::supervise(&actor_reference, store)"));
    assert!(source.contains(".ask(ledger::ExecuteEnvelope"));
    assert!(!source.contains("message.envelope.execute(&self.store)"));
}

#[test]
fn runtime_messages_cannot_be_empty_markers() {
    let mut violations = Vec::new();
    for file in SourceTree::new()
        .source_files()
        .into_iter()
        .map(SourceFile::read)
    {
        if file.is_guard_source() {
            continue;
        }
        for (line_index, line) in file.content.lines().enumerate() {
            let trimmed = line.trim();
            let is_struct_marker =
                trimmed.starts_with("pub struct ") || trimmed.starts_with("struct ");
            if is_struct_marker && trimmed.ends_with(';') && !trimmed.contains('(') {
                violations.push(format!(
                    "{}:{} declares empty marker {}",
                    file.path.display(),
                    line_index + 1,
                    trimmed
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "empty runtime-message marker violations:\n{}",
        violations.join("\n")
    );
}

#[tokio::test]
async fn message_daemon_cannot_skip_known_ledger_state() {
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
    let daemon = DaemonRoot::start(store).await;

    let response = daemon
        .ask(ExecuteEnvelope {
            envelope: DaemonEnvelope::Request(DaemonRequest::from_input(
                std::process::id(),
                Input::Agents(Agents {}),
            )),
        })
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
        .ask(ReadRootRequestCount {
            probe: RequestCountProbe::expecting_at_least(1),
        })
        .await
        .expect("daemon actor count reads through typed message");
    let store_count = daemon
        .ask(ReadLedgerRequestCount {
            probe: RequestCountProbe::expecting_at_least(1),
        })
        .await
        .expect("store actor count reads through typed message");

    assert_eq!(daemon_count.observed(), 1);
    assert_eq!(store_count.observed(), 1);
    assert!(daemon_count.satisfied());
    assert!(store_count.satisfied());

    daemon.stop_gracefully().await.expect("daemon actor stops");
    daemon.wait_for_shutdown().await;
}
