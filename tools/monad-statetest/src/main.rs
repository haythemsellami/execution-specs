use clap::Parser;
use monad_statetest::run_file;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "monad-statetest")]
#[command(version)]
#[command(about = "Run MONAD_EIGHT state fixtures directly against monad-revm")]
struct Cli {
    #[arg(required = true)]
    fixture: PathBuf,
    #[arg(long, help = "Emit machine-readable JSON results")]
    json: bool,
    #[arg(long, help = "Emit an EIP-3155 trace to stderr while executing")]
    trace: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let outcomes = run_file(&cli.fixture, cli.trace)?;

    if cli.json {
        println!("{}", serde_json::to_string(&outcomes)?);
    } else {
        for outcome in outcomes {
            if outcome.pass {
                println!("PASS {}", outcome.name);
            } else {
                println!("FAIL {} {}", outcome.name, outcome.error);
            }
        }
    }

    Ok(())
}
