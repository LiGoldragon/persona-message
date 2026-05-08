use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use crate::error::{Error, Result};
use crate::resolver::{ActorIndex, ProcessAncestry};
use crate::schema::{Actor, ActorId, Message};
use persona_wezterm::pty::PtySocket;
use persona_wezterm::terminal::{TerminalPrompt, WezTermMux};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorePath {
    root: PathBuf,
}

impl StorePath {
    pub fn from_environment() -> Self {
        match std::env::var_os("PERSONA_MESSAGE_STORE") {
            Some(path) => Self {
                root: PathBuf::from(path),
            },
            None => Self::from_path(".persona-message"),
        }
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self { root: path.into() }
    }

    pub fn root(&self) -> &Path {
        self.root.as_path()
    }

    pub fn actor_index(&self) -> PathBuf {
        self.root.join("actors.nota")
    }

    pub fn message_log(&self) -> PathBuf {
        self.root.join("messages.nota.log")
    }
}

#[derive(Debug, Clone)]
pub struct MessageStore {
    path: StorePath,
}

impl MessageStore {
    pub fn from_path(path: StorePath) -> Self {
        Self { path }
    }

    pub fn from_environment() -> Self {
        Self::from_path(StorePath::from_environment())
    }

    pub fn path(&self) -> &StorePath {
        &self.path
    }

    pub fn actors(&self) -> Result<ActorIndex> {
        ActorIndex::load(&self.path.actor_index())
    }

    pub fn registration_pid(&self) -> Result<u32> {
        ProcessAncestry::current()?.registration_pid()
    }

    pub fn register(&self, actor: &Actor) -> Result<()> {
        std::fs::create_dir_all(self.path.root())?;
        let mut index = ActorIndex::load_or_empty(&self.path.actor_index())?;
        index.upsert(actor.clone());
        let text = index
            .actors()
            .iter()
            .map(Actor::to_nota)
            .collect::<std::result::Result<Vec<_>, _>>()?
            .join("\n");
        std::fs::write(self.path.actor_index(), format!("{text}\n"))?;
        Ok(())
    }

    pub fn resolve_sender(&self) -> Result<ActorId> {
        let ancestry = ProcessAncestry::current()?;
        self.resolve_sender_from_ancestry(&ancestry)
    }

    pub fn resolve_sender_from_ancestry(&self, ancestry: &ProcessAncestry) -> Result<ActorId> {
        self.actors()?
            .resolve(ancestry)
            .ok_or_else(|| Error::NoMatchingAgent {
                path: self.path.actor_index(),
            })
    }

    pub fn append(&self, message: &Message) -> Result<()> {
        std::fs::create_dir_all(self.path.root())?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.path.message_log())?;
        let mut line = message.to_nota()?;
        line.push('\n');
        file.write_all(line.as_bytes())?;
        Ok(())
    }

    pub fn deliver(&self, message: &Message) -> Result<bool> {
        let actors = self.actors()?;
        let Some(actor) = actors.actor(&message.to) else {
            return Ok(false);
        };
        let prompt = TerminalPrompt::from_text(message.to_nota()?);
        actor.deliver(&prompt)
    }

    pub fn messages(&self) -> Result<Vec<Message>> {
        let path = self.path.message_log();
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };

        let mut messages = Vec::new();
        for (index, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let message = Message::from_nota(line).map_err(|source| Error::InvalidStoreLine {
                path: path.clone(),
                line: index + 1,
                source,
            })?;
            messages.push(message);
        }
        Ok(messages)
    }

    pub fn next_sequence(&self) -> Result<u64> {
        Ok(self.messages()?.len() as u64 + 1)
    }

    pub fn inbox(&self, recipient: &ActorId) -> Result<Vec<Message>> {
        Ok(self
            .messages()?
            .into_iter()
            .filter(|message| &message.to == recipient)
            .collect())
    }

    pub fn tail(&self, recipient: &ActorId, mut output: impl Write) -> Result<()> {
        std::fs::create_dir_all(self.path.root())?;
        let path = self.path.message_log();
        let mut offset = OpenOptions::new()
            .read(true)
            .create(true)
            .append(true)
            .open(&path)?
            .seek(SeekFrom::End(0))?;

        loop {
            let text = std::fs::read_to_string(&path)?;
            if offset as usize > text.len() {
                offset = 0;
            }
            let tail = &text[offset as usize..];
            for line in tail.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                let message =
                    Message::from_nota(line).map_err(|source| Error::InvalidStoreLine {
                        path: path.clone(),
                        line: 0,
                        source,
                    })?;
                if &message.to == recipient {
                    writeln!(output, "{}", message.to_nota()?)?;
                    output.flush()?;
                }
            }
            offset = text.len() as u64;
            thread::sleep(Duration::from_millis(200));
        }
    }
}

impl Actor {
    pub fn deliver(&self, prompt: &TerminalPrompt) -> Result<bool> {
        let Some(endpoint) = &self.endpoint else {
            return Ok(false);
        };
        match endpoint.kind.as_str() {
            "human" => Ok(false),
            "pty-socket" => {
                PtySocket::from_path(&endpoint.target).send_prompt(prompt.as_str())?;
                Ok(true)
            }
            "wezterm-pane" => {
                let pane_id = endpoint.target.parse().map_err(|_| Error::InvalidPaneId {
                    got: endpoint.target.clone(),
                })?;
                let mux = match &endpoint.aux {
                    Some(socket) => WezTermMux::from_environment().with_socket(socket),
                    None => WezTermMux::from_environment(),
                };
                mux.pane(pane_id).deliver(&prompt)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}
