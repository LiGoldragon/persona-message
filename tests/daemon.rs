use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use persona_message::store::{MessageStore, StorePath};

#[derive(Debug)]
struct DaemonFixture {
    directory: tempfile::TempDir,
    store: std::path::PathBuf,
    socket: std::path::PathBuf,
    daemon: Child,
}

impl DaemonFixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = directory.path().join("store");
        let socket = directory.path().join("message.sock");
        std::fs::create_dir_all(&store).expect("store directory");
        let daemon = Command::new(env!("CARGO_BIN_EXE_message-daemon"))
            .arg(&socket)
            .env("PERSONA_MESSAGE_STORE", &store)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("message daemon starts");
        let fixture = Self {
            directory,
            store,
            socket,
            daemon,
        };
        fixture.wait_for_socket();
        fixture
    }

    fn wait_for_socket(&self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if self.socket.exists() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!(
            "message daemon socket did not appear: {}",
            self.socket.display()
        );
    }

    fn spawn_sender(&self, start: &std::path::Path, input: &str) -> Child {
        Command::new("sh")
            .arg("-c")
            .arg(format!(
                "while [ ! -f '{}' ]; do sleep 0.05; done; '{}' '{}'",
                start.display(),
                env!("CARGO_BIN_EXE_message"),
                input
            ))
            .env("PERSONA_MESSAGE_STORE", &self.store)
            .env("PERSONA_MESSAGE_DAEMON", &self.socket)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("sender process starts")
    }

    fn write_actors(&self, actors: &[(&str, u32)]) {
        let text = actors
            .iter()
            .map(|(name, pid)| format!("(Actor {name} {pid} None)"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(self.store.join("actors.nota"), format!("{text}\n"))
            .expect("actor index writes");
    }

    fn messages(&self) -> persona_message::Result<Vec<persona_message::schema::Message>> {
        MessageStore::from_path(StorePath::from_path(&self.store)).messages()
    }
}

impl Drop for DaemonFixture {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

#[test]
fn cli_clients_route_messages_through_daemon_actor_state() {
    let fixture = DaemonFixture::new();
    let start = fixture.directory.path().join("start");
    let operator = fixture.spawn_sender(&start, "(Send designer hello)");
    let designer = fixture.spawn_sender(&start, "(Send operator reply)");
    fixture.write_actors(&[("operator", operator.id()), ("designer", designer.id())]);

    std::fs::write(&start, "").expect("start marker writes");
    let operator_output = operator.wait_with_output().expect("operator exits");
    let designer_output = designer.wait_with_output().expect("designer exits");

    assert!(operator_output.status.success());
    assert!(designer_output.status.success());

    let messages = fixture.messages().expect("messages read");
    assert!(messages.iter().any(|message| {
        message.from.as_str() == "operator"
            && message.to.as_str() == "designer"
            && message.body == "hello"
    }));
    assert!(messages.iter().any(|message| {
        message.from.as_str() == "designer"
            && message.to.as_str() == "operator"
            && message.body == "reply"
    }));
}
