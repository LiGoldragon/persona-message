use std::path::Path;

use crate::error::{Error, Result};
use crate::schema::{Actor, ActorId};

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

    pub fn actor(&self, actor: &ActorId) -> Option<&Actor> {
        self.actors.iter().find(|entry| &entry.name == actor)
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
