use clap::Parser;
use colored::*;
use std::env;
use std::error::Error;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{channel, Sender};
use std::thread::{self, JoinHandle};

#[cfg(not(target_os = "windows"))]
use std::os::unix::process::ExitStatusExt;

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
    child: Child,
    stdout_tx_handle: Option<JoinHandle<()>>,
    stderr_tx_handle: Option<JoinHandle<()>>,
}

fn spawn(
    arg: &String,
    stdout_tx: Sender<String>,
    stderr_tx: Sender<String>,
    stdout_prefix: ColoredString,
    stderr_prefix: ColoredString,
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
                    "{} {}",
                    stdout_prefix,
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
                    "{} {}",
                    stderr_prefix,
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

fn check_taskkill() -> Result<(), Box<dyn Error>> {
    Command::new("taskkill")
        .arg("/?")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    Ok(())
}

fn kill_child(child: &mut Child) -> Result<(), Box<dyn Error>> {
    if cfg!(target_os = "windows") {
        Command::new("taskkill")
            .arg("/F")
            .arg("/T")
            .arg("/PID")
            .arg(child.id().to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
    } else {
        child.kill()?;
    }
    Ok(())
}

fn process_exit_status_fallback(exit_status: ExitStatus) -> i32 {
    #[cfg(target_os = "windows")]
    {
        exit_status.code().unwrap_or_else(|| {
            eprintln!(
                "{}",
                "error attempting to get exit code from check command".bright_red()
            );
            1
        })
    }

    #[cfg(not(target_os = "windows"))]
    {
        match exit_status.signal() {
            Some(signal) => {
                eprintln!(
                    "{}",
                    format!("check command exited with signal: {signal}").bright_red()
                );
                128 + signal
            }
            None => {
                eprintln!(
                    "{}",
                    "error attempting to get exit code or signal from check command".bright_red()
                );
                1
            }
        }
    }
}

fn cleanup(
    run: &mut Spawn,
    check: &mut Spawn,
    stdout_tx: Sender<String>,
    stderr_tx: Sender<String>,
    rx_handle: thread::JoinHandle<()>,
) {
    kill_child(&mut run.child).expect("failed to kill run command");
    kill_child(&mut check.child).expect("failed to kill check command");

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
    rx_handle.join().expect("failed to join rx thread");
}

fn main() -> Result<(), Box<dyn Error>> {
    if cfg!(target_os = "windows") {
        check_taskkill().expect("taskkill command must be available");
    }

    let cli = Cli::parse();
    let (stdout_tx, stdout_rx) = channel();
    let (stderr_tx, stderr_rx) = channel();

    let mut run = spawn(
        &cli.run,
        stdout_tx.clone(),
        stderr_tx.clone(),
        "RUN".bright_green().bold(),
        "RUN".bright_yellow().bold(),
    )?;
    let mut check = spawn(
        &cli.check,
        stdout_tx.clone(),
        stderr_tx.clone(),
        "CHECK".bright_blue().bold(),
        "CHECK".bright_red().bold(),
    )?;

    let rx_handle = thread::spawn(move || loop {
        match stdout_rx.try_recv() {
            Ok(line) => println!("{}", line),
            Err(e) => match e {
                std::sync::mpsc::TryRecvError::Empty => (),
                std::sync::mpsc::TryRecvError::Disconnected => break,
            },
        }

        match stderr_rx.try_recv() {
            Ok(line) => eprintln!("{}", line),
            Err(e) => match e {
                std::sync::mpsc::TryRecvError::Empty => (),
                std::sync::mpsc::TryRecvError::Disconnected => break,
            },
        }
    });

    let exit_code;
    loop {
        if let Some(status) = check.child.try_wait()? {
            match status.code() {
                Some(code) => {
                    if code == 0 {
                        println!(
                            "{}",
                            format!("check command exited with code: {code}").bright_green()
                        );
                    } else {
                        eprintln!(
                            "{}",
                            format!("check command exited with code: {code}").bright_red()
                        );
                        exit_code = code;
                        break;
                    }
                }
                None => {
                    exit_code = process_exit_status_fallback(status);
                    break;
                }
            }
        }

        if let Some(status) = run.child.try_wait()? {
            match status.code() {
                Some(code) => {
                    if code == 0 {
                        println!(
                            "{}",
                            format!("run command exited with code: {code}").bright_green()
                        );
                    } else {
                        eprintln!(
                            "{}",
                            format!("run command exited with code: {code}").bright_red()
                        );
                    }

                    exit_code = code;
                    break;
                }
                None => {
                    exit_code = process_exit_status_fallback(status);
                    break;
                }
            }
        }
    }

    cleanup(&mut run, &mut check, stdout_tx, stderr_tx, rx_handle);
    std::process::exit(exit_code);
}
