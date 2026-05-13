use persona_message::Result;
use persona_message::daemon::MessageDaemonCommandLine;

fn main() -> Result<()> {
    MessageDaemonCommandLine::from_env().run()
}
