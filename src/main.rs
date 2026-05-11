use persona_message::Result;
use persona_message::command::CommandLine;

fn main() -> Result<()> {
    let command_line = CommandLine::from_env();
    command_line.run(std::io::stdout().lock())?;
    Ok(())
}
