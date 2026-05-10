use kameo::actor::{Actor, ActorRef};
use kameo::error::Infallible;
use kameo::message::{Context, Message};

use crate::Result;
use crate::daemon::{ActorRequestCount, ActorRequestCountProbe, DaemonEnvelope};
use crate::store::MessageStore;

pub struct MessageStoreActor {
    store: MessageStore,
    executed_request_count: u64,
}

impl MessageStoreActor {
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

impl Actor for MessageStoreActor {
    type Args = MessageStore;
    type Error = Infallible;

    async fn on_start(
        store: Self::Args,
        _actor_reference: ActorRef<Self>,
    ) -> std::result::Result<Self, Self::Error> {
        Ok(Self::new(store))
    }
}

pub struct ExecuteStoreEnvelope {
    pub envelope: DaemonEnvelope,
}

impl Message<ExecuteStoreEnvelope> for MessageStoreActor {
    type Reply = Result<DaemonEnvelope>;

    async fn handle(
        &mut self,
        message: ExecuteStoreEnvelope,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.execute_envelope(message.envelope)
    }
}

pub struct ReadStoreActorRequestCount {
    pub probe: ActorRequestCountProbe,
}

impl Message<ReadStoreActorRequestCount> for MessageStoreActor {
    type Reply = ActorRequestCount;

    async fn handle(
        &mut self,
        message: ReadStoreActorRequestCount,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        message.probe.inspect(self.request_count())
    }
}
