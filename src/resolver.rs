use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::schema::{Actor, ActorId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorIndexPath {
    path: PathBuf,
}

impl ActorIndexPath {
    pub fn from_environment() -> Self {
        if let Some(path) = std::env::var_os("PERSONA_MESSAGE_ACTORS") {
            return Self::from_path(path);
        }

        let root = std::env::var_os("PERSONA_MESSAGE_STORE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(".persona-message"));
        Self::from_path(root.join("actors.nota"))
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub fn load(&self) -> Result<ActorIndex> {
        ActorIndex::load(self.path())
    }

    pub fn resolve_current_process(&self) -> Result<ActorId> {
        let ancestry = ProcessAncestry::current()?;
        self.load()?
            .resolve(&ancestry)
            .ok_or_else(|| Error::NoMatchingAgent {
                path: self.path.clone(),
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorIndex {
    actors: Vec<Actor>,
}

impl ActorIndex {
    pub fn from_actors(actors: Vec<Actor>) -> Self {
        Self { actors }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Self::from_text(path, &text)
    }

    fn from_text(path: &Path, text: &str) -> Result<Self> {
        let mut actors = Vec::new();
        for (index, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let actor = Actor::from_nota(line).map_err(|source| Error::InvalidActorLine {
                path: path.to_path_buf(),
                line: index + 1,
                source,
            })?;
            actors.push(actor);
        }
        Ok(Self { actors })
    }

    pub fn resolve(&self, ancestry: &ProcessAncestry) -> Option<ActorId> {
        ancestry.pids().iter().find_map(|pid| {
            self.actors
                .iter()
                .find(|actor| actor.pid == *pid)
                .map(|actor| actor.name.clone())
        })
    }

    pub fn actors(&self) -> &[Actor] {
        self.actors.as_slice()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessAncestry {
    pids: Vec<u32>,
}

impl ProcessAncestry {
    pub fn current() -> Result<Self> {
        Self::from_process(std::process::id())
    }

    pub fn from_process(mut pid: u32) -> Result<Self> {
        let mut pids = Vec::new();
        loop {
            pids.push(pid);
            let parent = parent_process(pid)?;
            if parent == 0 || parent == pid {
                break;
            }
            pid = parent;
        }
        Ok(Self { pids })
    }

    pub fn from_pids(pids: Vec<u32>) -> Self {
        Self { pids }
    }

    pub fn pids(&self) -> &[u32] {
        self.pids.as_slice()
    }
}

fn parent_process(pid: u32) -> Result<u32> {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status"))?;
    for line in status.lines() {
        let Some(rest) = line.strip_prefix("PPid:") else {
            continue;
        };
        let text = rest.trim();
        return text.parse().map_err(|_| Error::InvalidProcessId {
            got: text.to_string(),
        });
    }
    Err(Error::MissingParentProcess { pid })
}
