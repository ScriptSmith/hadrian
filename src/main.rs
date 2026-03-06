#[cfg(feature = "cli")]
use clap::Parser;

#[cfg(feature = "cli")]
#[tokio::main]
async fn main() {
    let args = hadrian::cli::Args::parse();
    hadrian::cli::dispatch(args).await;
}

#[cfg(not(feature = "cli"))]
fn main() {
    eprintln!(
        "The CLI feature is not enabled. Build with --features cli (or server/tiny/minimal/standard/full)."
    );
    std::process::exit(1);
}
