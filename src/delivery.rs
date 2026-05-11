use crate::error::Result;
use crate::schema::{Actor, EndpointKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryOutcome {
    state: DeliveryState,
}

impl DeliveryOutcome {
    pub fn delivered() -> Self {
        Self {
            state: DeliveryState::Delivered,
        }
    }

    pub fn deferred(reason: DeliveryDeferral) -> Self {
        Self {
            state: DeliveryState::Deferred(reason),
        }
    }

    pub fn unreachable() -> Self {
        Self {
            state: DeliveryState::Unreachable,
        }
    }

    pub fn delivered_to_terminal(&self) -> bool {
        matches!(self.state, DeliveryState::Delivered)
    }

    pub fn deferred_delivery(&self) -> bool {
        matches!(self.state, DeliveryState::Deferred(_))
    }

    pub fn state(&self) -> &DeliveryState {
        &self.state
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryState {
    Delivered,
    Deferred(DeliveryDeferral),
    Unreachable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryDeferral {
    RouterRequired { endpoint: EndpointKind },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryGate {}

impl DeliveryGate {
    pub fn from_environment() -> Self {
        Self {}
    }

    pub fn deliver(&self, actor: &Actor, _prompt_text: &str) -> Result<DeliveryOutcome> {
        let Some(endpoint) = &actor.endpoint else {
            return Ok(DeliveryOutcome::unreachable());
        };

        match endpoint.kind {
            EndpointKind::Human => Ok(DeliveryOutcome::unreachable()),
            EndpointKind::PtySocket => Ok(DeliveryOutcome::deferred(
                DeliveryDeferral::RouterRequired {
                    endpoint: endpoint.kind,
                },
            )),
        }
    }
}

impl Actor {
    pub fn deliver(&self, prompt_text: &str) -> Result<bool> {
        Ok(DeliveryGate::from_environment()
            .deliver(self, prompt_text)?
            .delivered_to_terminal())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptState {
    Empty,
    Occupied { preview: String },
}

impl PromptState {
    pub fn from_cursor_line(line: &str, cursor_column: u16) -> Self {
        let prefix = line
            .chars()
            .take(cursor_column as usize)
            .collect::<String>();
        let text = prefix
            .trim()
            .trim_start_matches('>')
            .trim_start_matches('›')
            .trim();
        if text.is_empty() {
            Self::Empty
        } else {
            Self::Occupied {
                preview: text.chars().take(80).collect(),
            }
        }
    }
}
