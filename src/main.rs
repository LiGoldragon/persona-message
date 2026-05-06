use persona_message::Result;
use persona_message::command::CommandLine;
use persona_message::store::MessageStore;

fn main() -> Result<()> {
    let command_line = CommandLine::from_env();
    let store = MessageStore::from_environment();
    command_line.run(&store, std::io::stdout().lock())?;
    Ok(())
}
