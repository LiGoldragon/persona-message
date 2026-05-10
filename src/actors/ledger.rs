use kameo::actor::{Actor, ActorRef};
use kameo::error::Infallible;
use kameo::message::{Context, Message};

use crate::Result;
use crate::daemon::{DaemonEnvelope, RequestCount, RequestCountProbe};
use crate::store::MessageStore;

pub struct Ledger {
    store: MessageStore,
    executed_request_count: u64,
}

impl Ledger {
    fn new(store: MessageStore) -> Self {
        Self {
            store,
            executed_request_count: 0,
        }
    }

    fn execute_envelope(&mut self, envelope: DaemonEnvelope) -> Result<DaemonEnvelope> {
        if matches!(envelope, DaemonEnvelope::Request(_)) {
            self.executed_request_count = self.executed_request_count.saturating_add(1);
        }
        envelope.execute(&self.store)
    }

    fn request_count(&self) -> u64 {
        self.executed_request_count
    }
}

impl Actor for Ledger {
    type Args = MessageStore;
    type Error = Infallible;

    async fn on_start(
        store: Self::Args,
        _actor_reference: ActorRef<Self>,
    ) -> std::result::Result<Self, Self::Error> {
        Ok(Self::new(store))
    }
}

pub struct ExecuteEnvelope {
    pub envelope: DaemonEnvelope,
}

impl Message<ExecuteEnvelope> for Ledger {
    type Reply = Result<DaemonEnvelope>;

    async fn handle(
        &mut self,
        message: ExecuteEnvelope,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.execute_envelope(message.envelope)
    }
}

pub struct ReadRequestCount {
    pub probe: RequestCountProbe,
}

impl Message<ReadRequestCount> for Ledger {
    type Reply = RequestCount;

    async fn handle(
        &mut self,
        message: ReadRequestCount,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        message.probe.inspect(self.request_count())
    }
}
