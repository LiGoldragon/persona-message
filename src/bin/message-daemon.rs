use persona_message::daemon::DaemonCommandLine;

fn main() -> persona_message::Result<()> {
    DaemonCommandLine::from_env().run()
}
