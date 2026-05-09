use persona_system::{NiriFocusSource, SystemTarget};
use persona_wezterm::pty::{PtyScreenSnapshot, PtySocket};
use persona_wezterm::terminal::{TerminalPrompt, WezTermMux};

use crate::error::{Error, Result};
use crate::schema::{Actor, EndpointKind, EndpointTransport};

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryState {
    Delivered,
    Deferred(DeliveryDeferral),
    Unreachable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryDeferral {
    Focused { window: u64 },
    PromptOccupied { preview: String },
    PromptUnknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryGate {
    focus: NiriFocusSource,
    screen: TerminalScreenGeometry,
}

impl DeliveryGate {
    pub fn from_environment() -> Self {
        Self {
            focus: NiriFocusSource::from_environment(),
            screen: TerminalScreenGeometry::from_environment(),
        }
    }

    pub fn deliver(&self, actor: &Actor, prompt: &TerminalPrompt) -> Result<DeliveryOutcome> {
        let Some(endpoint) = &actor.endpoint else {
            return Ok(DeliveryOutcome::unreachable());
        };

        match endpoint.kind {
            EndpointKind::Human => Ok(DeliveryOutcome::unreachable()),
            EndpointKind::PtySocket => self.deliver_to_pty_socket(endpoint, prompt),
            EndpointKind::WezTermPane => self.deliver_to_wezterm_pane(endpoint, prompt),
        }
    }

    fn deliver_to_pty_socket(
        &self,
        endpoint: &EndpointTransport,
        prompt: &TerminalPrompt,
    ) -> Result<DeliveryOutcome> {
        let socket = PtySocket::from_path(&endpoint.target);
        if let Some(window) = endpoint.niri_window_target()? {
            let observation = self
                .focus
                .observe(SystemTarget::niri_window(window.value()))?;
            if observation.focused {
                return Ok(DeliveryOutcome::deferred(DeliveryDeferral::Focused {
                    window: window.value(),
                }));
            }
            let screen = socket
                .capture()?
                .screen(self.screen.rows, self.screen.columns);
            match PromptState::from_screen(&screen) {
                PromptState::Empty => {}
                PromptState::Occupied { preview } => {
                    return Ok(DeliveryOutcome::deferred(
                        DeliveryDeferral::PromptOccupied { preview },
                    ));
                }
                PromptState::Unknown => {
                    return Ok(DeliveryOutcome::deferred(DeliveryDeferral::PromptUnknown));
                }
            }
        }
        socket.send_prompt(prompt.as_str())?;
        let capture = socket.capture()?.to_string_lossy();
        if !capture.contains(prompt.as_str()) {
            return Ok(DeliveryOutcome::deferred(DeliveryDeferral::PromptUnknown));
        }
        Ok(DeliveryOutcome::delivered())
    }

    fn deliver_to_wezterm_pane(
        &self,
        endpoint: &EndpointTransport,
        prompt: &TerminalPrompt,
    ) -> Result<DeliveryOutcome> {
        let pane_id = endpoint.target.parse().map_err(|_| Error::InvalidPaneId {
            got: endpoint.target.clone(),
        })?;
        let mux = match &endpoint.aux {
            Some(socket) => WezTermMux::from_environment().with_socket(socket),
            None => WezTermMux::from_environment(),
        };
        mux.pane(pane_id).deliver(prompt)?;
        Ok(DeliveryOutcome::delivered())
    }
}

impl Actor {
    pub fn deliver(&self, prompt: &TerminalPrompt) -> Result<bool> {
        Ok(DeliveryGate::from_environment()
            .deliver(self, prompt)?
            .delivered_to_terminal())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NiriWindowTarget {
    value: u64,
}

impl NiriWindowTarget {
    fn from_text(text: &str) -> Result<Self> {
        let value = text.strip_prefix("niri-window:").unwrap_or(text);
        let value = value.parse().map_err(|_| Error::InvalidNiriWindowTarget {
            got: text.to_string(),
        })?;
        Ok(Self { value })
    }

    pub fn value(self) -> u64 {
        self.value
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TerminalScreenGeometry {
    rows: u16,
    columns: u16,
}

impl TerminalScreenGeometry {
    fn from_environment() -> Self {
        let rows = std::env::var("PERSONA_WEZTERM_CAPTURE_ROWS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(32);
        let columns = std::env::var("PERSONA_WEZTERM_CAPTURE_COLUMNS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(120);
        Self { rows, columns }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptState {
    Empty,
    Occupied { preview: String },
    Unknown,
}

impl PromptState {
    pub fn from_screen(screen: &PtyScreenSnapshot) -> Self {
        Self::from_cursor_line(screen.cursor_line(), screen.cursor_column())
    }

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

impl EndpointTransport {
    pub fn niri_window_target(&self) -> Result<Option<NiriWindowTarget>> {
        self.aux
            .as_deref()
            .map(NiriWindowTarget::from_text)
            .transpose()
    }
}
