mod cli;
mod process;
mod status_parse;

pub use cli::TailscaleCli;
pub use process::DaemonState;
pub use status_parse::NetworkStatus;
