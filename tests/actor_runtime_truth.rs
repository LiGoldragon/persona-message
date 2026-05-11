use std::fs;
use std::path::PathBuf;

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
        let mut files = vec![
            self.root.join("Cargo.toml"),
            self.root.join("Cargo.lock"),
            self.root.join("flake.nix"),
        ];
        files.extend(self.source_files());
        files.extend(self.test_files());
        files
            .into_iter()
            .filter(|path| path.exists())
            .map(SourceFile::read)
            .collect()
    }

    fn source_files(&self) -> Vec<PathBuf> {
        self.rust_files_below(self.root.join("src"))
    }

    fn test_files(&self) -> Vec<PathBuf> {
        self.rust_files_below(self.root.join("tests"))
    }

    fn rust_files_below(&self, root: PathBuf) -> Vec<PathBuf> {
        if !root.exists() {
            return Vec::new();
        }
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
fn proxy_has_no_actor_runtime() {
    let forbidden_fragments = [
        "kameo =",
        "name = \"kameo\"",
        "tokio =",
        "name = \"tokio\"",
        "async-std",
        "ractor =",
        "name = \"ractor\"",
        "use kameo",
        "kameo::",
        "use tokio",
        "tokio::",
        "use ractor",
        "ractor::",
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
        "proxy actor-runtime violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn proxy_has_no_daemon_binary() {
    let forbidden_fragments = [
        "message-daemon",
        "PERSONA_MESSAGE_DAEMON",
        "DaemonRoot",
        "DaemonEnvelope",
        "MessageDaemon",
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
        "proxy daemon-surface violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn proxy_input_enum_has_exact_destination_variants() {
    let source = SourceFile::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("command.rs"),
    );

    assert!(source.contains("pub enum Input {"));
    assert!(source.contains("Send(Send),"));
    assert!(source.contains("Inbox(Inbox),"));
    for retired_variant in ["Tail", "Register", "Agents", "Flush"] {
        assert!(
            !source.contains(retired_variant),
            "Input source still contains retired variant {retired_variant}"
        );
    }
}

#[test]
fn proxy_cannot_open_local_message_ledger() {
    let forbidden_fragments = [
        "messages.nota.log",
        "pending.nota.log",
        "MessageStore",
        "StorePath",
        "DeliveryGate",
        "DeliveryOutcome",
        "DeliveryDeferral",
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
        "proxy local-ledger violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn proxy_endpoint_vocabulary_is_not_owned_here() {
    let forbidden_fragments = [
        "EndpointTransport",
        "EndpointKind",
        "PtySocket",
        "Human",
        "Attachment",
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
        "proxy endpoint-vocabulary violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn proxy_constructs_no_in_band_proof_material() {
    let forbidden_fragments = ["AuthProof", "LocalOperatorProof", ".with_auth("];

    let mut violations = Vec::new();
    for file in SourceTree::new()
        .source_files()
        .into_iter()
        .map(SourceFile::read)
    {
        for fragment in forbidden_fragments {
            if file.contains(fragment) {
                violations.push(format!("{} contains {fragment}", file.path.display()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "proxy proof-material violations:\n{}",
        violations.join("\n")
    );
}
