//! A harness child process: write messages to its stdin, read its stdout on a background thread.

use crate::KitConfig;
use harness_protocol::{read_message, write_message, ReadError, RpcMessage};
use std::io::BufReader;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

enum Incoming {
    Message(RpcMessage),
    Malformed(String),
}

/// The result of waiting for the harness's next line.
pub enum Recv {
    Message(RpcMessage),
    Malformed(String),
    /// The harness closed its stdout.
    Eof,
    Timeout,
}

pub struct Session {
    child: Child,
    stdin: Option<ChildStdin>,
    rx: Receiver<Incoming>,
}

impl Session {
    pub fn spawn(cfg: &KitConfig) -> Result<Self, String> {
        let (program, args) = cfg.command.split_first().ok_or("empty harness command")?;
        let mut child = Command::new(program)
            .args(args)
            .envs(cfg.env.clone())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("{program}: {e}"))?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().expect("stdout is piped");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let item = match read_message(&mut reader) {
                    Ok(msg) => Incoming::Message(msg),
                    Err(ReadError::Malformed { line, .. }) => Incoming::Malformed(line),
                    Err(ReadError::Eof) | Err(ReadError::Io(_)) => break,
                };
                if tx.send(item).is_err() {
                    break;
                }
            }
        });
        Ok(Self { child, stdin, rx })
    }

    /// False if the harness's stdin is closed or the write failed.
    pub fn send(&mut self, msg: &RpcMessage) -> bool {
        match self.stdin.as_mut() {
            Some(stdin) => write_message(stdin, msg).is_ok(),
            None => false,
        }
    }

    /// Closing stdin is the protocol's "shut down" signal.
    pub fn close_stdin(&mut self) {
        self.stdin = None;
    }

    pub fn recv(&self, timeout: Duration) -> Recv {
        match self.rx.recv_timeout(timeout) {
            Ok(Incoming::Message(msg)) => Recv::Message(msg),
            Ok(Incoming::Malformed(line)) => Recv::Malformed(line),
            Err(RecvTimeoutError::Timeout) => Recv::Timeout,
            Err(RecvTimeoutError::Disconnected) => Recv::Eof,
        }
    }

    /// True if the process exited within `timeout`.
    pub fn wait_exit(&mut self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if let Ok(Some(_)) = self.child.try_wait() {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.kill();
    }
}
