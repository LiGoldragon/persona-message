use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use persona_message::store::{MessageStore, StorePath};

fn shell_send(
    store: &std::path::Path,
    start: &std::path::Path,
    input: &str,
) -> std::process::Child {
    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "while [ ! -f '{}' ]; do sleep 0.05; done; PERSONA_MESSAGE_STORE='{}' '{}' '{}'",
            start.display(),
            store.display(),
            env!("CARGO_BIN_EXE_message"),
            input
        ))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn shell sender")
}

fn write_agents(store: &std::path::Path, agents: &[(&str, u32)]) {
    let text = agents
        .iter()
        .map(|(name, pid)| format!("(Actor {name} {pid} None)"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::create_dir_all(store).expect("store directory");
    std::fs::write(store.join("actors.nota"), format!("{text}\n")).expect("actor index writes");
}

#[test]
fn two_processes_exchange_messages_with_resolved_senders() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = directory.path().join("store");
    let start = directory.path().join("start");
    let operator = shell_send(&store, &start, r#"(Send designer "hello from operator")"#);
    let designer = shell_send(&store, &start, r#"(Send operator "reply from designer")"#);
    write_agents(
        &store,
        &[("operator", operator.id()), ("designer", designer.id())],
    );

    std::fs::write(&start, "").expect("start marker writes");
    let operator_output = operator.wait_with_output().expect("operator exits");
    let designer_output = designer.wait_with_output().expect("designer exits");

    assert!(operator_output.status.success());
    assert!(designer_output.status.success());

    let store = MessageStore::from_path(StorePath::from_path(&store));
    let messages = store.messages().expect("messages read");

    assert!(messages.iter().any(|message| {
        message.from.as_str() == "operator"
            && message.to.as_str() == "designer"
            && message.body == "hello from operator"
    }));
    assert!(messages.iter().any(|message| {
        message.from.as_str() == "designer"
            && message.to.as_str() == "operator"
            && message.body == "reply from designer"
    }));
}

#[test]
fn tail_prints_messages_for_resolved_recipient() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = directory.path().join("store");
    let tail_start = directory.path().join("tail-start");
    let send_start = directory.path().join("send-start");
    let mut tail = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "while [ ! -f '{}' ]; do sleep 0.05; done; PERSONA_MESSAGE_STORE='{}' '{}' '(Tail)'",
            tail_start.display(),
            store.display(),
            env!("CARGO_BIN_EXE_message")
        ))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tail");
    let sender = shell_send(
        &store,
        &send_start,
        r#"(Send designer "tail-visible message")"#,
    );
    write_agents(
        &store,
        &[("designer", tail.id()), ("operator", sender.id())],
    );

    let stdout = tail.stdout.take().expect("tail stdout");
    let (send_line, receive_line) = mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        if reader.read_line(&mut line).is_ok() {
            let _ = send_line.send(line);
        }
    });

    std::fs::write(&tail_start, "").expect("tail start writes");
    std::thread::sleep(Duration::from_millis(300));
    std::fs::write(&send_start, "").expect("send start writes");

    let sender_output = sender.wait_with_output().expect("sender exits");
    assert!(sender_output.status.success());
    let line = receive_line
        .recv_timeout(Duration::from_secs(5))
        .expect("tail prints a matching message");

    assert!(line.contains("tail-visible message"));
    let _ = tail.kill();
    let _ = tail.wait();
}
