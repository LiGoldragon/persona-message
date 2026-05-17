use persona_message::Result;
use persona_message::output_validator::OutputValidatorCommandLine;

fn main() -> Result<()> {
    OutputValidatorCommandLine::from_environment().run()
}
