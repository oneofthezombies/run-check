use std::process::Command;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// command to run
    command_to_run: String,

    /// command to check
    #[arg(long, value_name = "COMMAND_TO_CHECK")]
    check: String,
}

fn main() {
    let cli = Cli::parse();
    println!("command: {:?}", cli.command_to_run);
    println!("check: {:?}", cli.check);

    // let mut child = Command::new("/bin/cat")
    // .arg("file.txt")
    // .spawn()
    // .expect("failed to execute child");

    // let ecode = child.wait().expect("failed to wait on child");

    // assert!(ecode.success());
}
