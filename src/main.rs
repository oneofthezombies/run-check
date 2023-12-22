use clap::Parser;
use std::env;
use std::error::Error;
use std::io::{BufRead, BufReader};
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Sender};
use std::thread;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// command to run
    #[arg(long, value_name = "COMMAND_TO_RUN")]
    run: String,

    /// command to check
    #[arg(long, value_name = "COMMAND_TO_CHECK")]
    check: String,
}

struct Spawn {
    child: std::process::Child,
    stdout_tx_handle: Option<thread::JoinHandle<()>>,
    stderr_tx_handle: Option<thread::JoinHandle<()>>,
}

fn spawn(
    prefix: &'static str,
    arg: &String,
    stdout_tx: Sender<String>,
    stderr_tx: Sender<String>,
) -> Result<Spawn, Box<dyn Error>> {
    let program = if cfg!(target_os = "windows") {
        env::var("COMSPEC").unwrap_or("cmd.exe".to_string())
    } else {
        env::var("SHELL").unwrap_or("/bin/sh".to_string())
    };

    let mut command = Command::new(program);
    if cfg!(target_os = "windows") {
        command.arg("/d").arg("/s").arg("/c");
    } else {
        command.arg("-c");
    }

    let mut child = command
        .arg(arg)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = BufReader::new(child.stdout.take().ok_or_else(|| "failed to open stdout")?);
    let stderr = BufReader::new(child.stderr.take().ok_or_else(|| "failed to open stderr")?);

    let stdout_tx_handle = thread::spawn(move || {
        for line in stdout.lines() {
            stdout_tx
                .send(format!(
                    "[{}] {}",
                    prefix,
                    line.expect("failed to read line from stdout")
                ))
                .unwrap();
        }
        drop(stdout_tx);
    });

    let stderr_tx_handle = thread::spawn(move || {
        for line in stderr.lines() {
            stderr_tx
                .send(format!(
                    "[{}] {}",
                    prefix,
                    line.expect("failed to read line from stderr")
                ))
                .unwrap();
        }
        drop(stderr_tx);
    });

    Ok(Spawn {
        child,
        stdout_tx_handle: Some(stdout_tx_handle),
        stderr_tx_handle: Some(stderr_tx_handle),
    })
}

fn cleanup(
    run: &mut Spawn,
    check: &mut Spawn,
    stdout_tx: Sender<String>,
    stderr_tx: Sender<String>,
    stdout_rx_handle: thread::JoinHandle<()>,
    stderr_rx_handle: thread::JoinHandle<()>,
) {
    run.child.kill().expect("failed to kill run command");
    check.child.kill().expect("failed to kill check command");

    run.stdout_tx_handle
        .take()
        .map(|h| h.join().expect("failed to join run stdout tx thread"));
    run.stderr_tx_handle
        .take()
        .map(|h| h.join().expect("failed to join run stderr tx thread"));
    check
        .stdout_tx_handle
        .take()
        .map(|h| h.join().expect("failed to join check stdout tx thread"));
    check
        .stderr_tx_handle
        .take()
        .map(|h| h.join().expect("failed to join check stderr tx thread"));

    drop(stdout_tx);
    drop(stderr_tx);
    stdout_rx_handle
        .join()
        .expect("failed to join stdout rx thread");
    stderr_rx_handle
        .join()
        .expect("failed to join stderr rx thread");
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let (stdout_tx, stdout_rx) = channel();
    let (stderr_tx, stderr_rx) = channel();

    let mut run = spawn("run", &cli.run, stdout_tx.clone(), stderr_tx.clone())?;
    let mut check = spawn("check", &cli.check, stdout_tx.clone(), stderr_tx.clone())?;

    let stdout_rx_handle = thread::spawn(move || {
        for line in stdout_rx {
            println!("{}", line);
        }
    });

    let stderr_rx_handle = thread::spawn(move || {
        for line in stderr_rx {
            eprintln!("{}", line);
        }
    });

    let exit_code;
    loop {
        if let Some(status) = check.child.try_wait()? {
            match status.code() {
                Some(code) => {
                    if code == 0 {
                        println!("check command exited with code: {code}");
                    } else {
                        eprintln!("check command exited with code: {code}");
                        exit_code = code;
                        break;
                    }
                }
                None => match status.signal() {
                    Some(signal) => {
                        eprintln!("check command exited with signal: {signal}");
                        exit_code = 128 + signal;
                        break;
                    }
                    None => {
                        eprintln!("error attempting to get exit code or signal from check command");
                        exit_code = 1;
                        break;
                    }
                },
            }
        }

        if let Some(status) = run.child.try_wait()? {
            match status.code() {
                Some(code) => {
                    if code == 0 {
                        println!("run command exited with code: {code}");
                    } else {
                        eprintln!("run command exited with code: {code}");
                    }

                    exit_code = code;
                    break;
                }
                None => match status.signal() {
                    Some(signal) => {
                        eprintln!("run command exited with signal: {signal}");
                        exit_code = 128 + signal;
                        break;
                    }
                    None => {
                        eprintln!("error attempting to get exit code or signal from run command");
                        exit_code = 1;
                        break;
                    }
                },
            }
        }
    }

    cleanup(
        &mut run,
        &mut check,
        stdout_tx,
        stderr_tx,
        stdout_rx_handle,
        stderr_rx_handle,
    );
    std::process::exit(exit_code);
}
