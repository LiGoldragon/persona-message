use persona_message::Result;
use persona_message::command::CommandLine;
use persona_message::resolver::ActorIndexPath;

fn main() -> Result<()> {
    let command_line = CommandLine::from_env();
    let actor_index_path = ActorIndexPath::from_environment();
    command_line.run(&actor_index_path, std::io::stdout().lock())?;
    Ok(())
}
