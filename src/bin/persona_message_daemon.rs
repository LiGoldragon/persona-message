use nota_config::ConfigurationSource;
use persona_message::Result;
use persona_message::daemon::MessageDaemon;
use signal_persona_message::MessageDaemonConfiguration;

fn main() -> Result<()> {
    let configuration: MessageDaemonConfiguration = ConfigurationSource::from_argv()?.decode()?;
    MessageDaemon::from_configuration(configuration).run()
}
