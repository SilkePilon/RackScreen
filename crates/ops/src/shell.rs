//! Command execution behind a trait so operations are testable without a Pi.

use std::collections::HashMap;
use std::process::Command;
use std::sync::Mutex;

use anyhow::{Context, Result};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Output {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    pub fn ok(stdout: &str) -> Output {
        Output {
            status: 0,
            stdout: stdout.into(),
            stderr: String::new(),
        }
    }
    pub fn fail(status: i32, stderr: &str) -> Output {
        Output {
            status,
            stdout: String::new(),
            stderr: stderr.into(),
        }
    }
    pub fn success(&self) -> bool {
        self.status == 0
    }
}

pub trait Shell: Send + Sync {
    fn run(&self, cmd: &str, args: &[&str]) -> Result<Output>;

    /// Run and turn a non-zero exit into an error carrying stderr.
    fn check(&self, cmd: &str, args: &[&str]) -> Result<Output> {
        let out = self.run(cmd, args)?;
        if !out.success() {
            anyhow::bail!(
                "{cmd} {} failed ({}): {}",
                args.join(" "),
                out.status,
                out.stderr.trim()
            );
        }
        Ok(out)
    }

    /// Run, handing each line of output (stdout, then stderr) to `line` as
    /// it comes; true on exit 0. The default waits for the whole output.
    fn run_streaming(&self, cmd: &str, args: &[&str], line: &mut dyn FnMut(&str)) -> Result<bool> {
        let out = self.run(cmd, args)?;
        out.stdout
            .lines()
            .chain(out.stderr.lines())
            .for_each(&mut *line);
        Ok(out.success())
    }
}

pub struct RealShell;

impl Shell for RealShell {
    fn run(&self, cmd: &str, args: &[&str]) -> Result<Output> {
        let out = Command::new(cmd)
            .args(args)
            .output()
            .with_context(|| format!("spawn {cmd}"))?;
        Ok(Output {
            status: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }

    fn run_streaming(&self, cmd: &str, args: &[&str], line: &mut dyn FnMut(&str)) -> Result<bool> {
        use std::process::Stdio;
        let mut child = Command::new(cmd)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("spawn {cmd}"))?;
        let stderr = child.stderr.take().expect("piped");
        let err_lines = std::thread::spawn(move || {
            let mut v = Vec::new();
            lossy_lines(stderr, |l| v.push(l));
            v
        });
        lossy_lines(child.stdout.take().expect("piped"), |l| line(&l));
        for l in err_lines.join().unwrap_or_default() {
            line(&l);
        }
        Ok(child.wait()?.success())
    }
}

/// Hand every line of `r` to `f`, without the line ending. Bytes that are
/// not UTF-8 become U+FFFD instead of ending the read (which would leave
/// the writer stuck on a full pipe).
pub fn lossy_lines(r: impl std::io::Read, mut f: impl FnMut(String)) {
    use std::io::BufRead;
    let mut r = std::io::BufReader::new(r);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match r.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                if buf.ends_with(b"\n") {
                    buf.pop();
                    if buf.ends_with(b"\r") {
                        buf.pop();
                    }
                }
                f(String::from_utf8_lossy(&buf).into_owned());
            }
        }
    }
}

/// Records every call and answers by longest matching prefix of "cmd arg arg ...".
#[derive(Default)]
pub struct FakeShell {
    calls: Mutex<Vec<String>>,
    responses: Mutex<HashMap<String, Output>>,
}

impl FakeShell {
    pub fn new() -> FakeShell {
        FakeShell::default()
    }
    pub fn respond(&self, prefix: &str, out: Output) {
        self.responses
            .lock()
            .unwrap()
            .insert(prefix.to_string(), out);
    }
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
    pub fn called(&self, prefix: &str) -> bool {
        self.calls().iter().any(|c| c.starts_with(prefix))
    }
}

impl Shell for FakeShell {
    fn run(&self, cmd: &str, args: &[&str]) -> Result<Output> {
        let line = std::iter::once(cmd)
            .chain(args.iter().copied())
            .collect::<Vec<_>>()
            .join(" ");
        self.calls.lock().unwrap().push(line.clone());
        let responses = self.responses.lock().unwrap();
        let best = responses
            .iter()
            .filter(|(k, _)| line.starts_with(k.as_str()))
            .max_by_key(|(k, _)| k.len());
        Ok(best
            .map(|(_, o)| o.clone())
            .unwrap_or_else(|| Output::ok("")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_records_and_answers_by_prefix() {
        let sh = FakeShell::new();
        sh.respond("systemctl is-active", Output::fail(3, "inactive"));
        assert!(sh.run("systemctl", &["is-active", "x"]).unwrap().status == 3);
        assert!(sh.run("systemctl", &["daemon-reload"]).unwrap().success());
        assert!(sh.called("systemctl daemon-reload"));
        assert!(sh.check("systemctl", &["is-active", "x"]).is_err());
    }

    #[test]
    fn real_shell_runs_true() {
        assert!(RealShell.run("true", &[]).unwrap().success());
        assert!(!RealShell.run("false", &[]).unwrap().success());
    }

    #[test]
    fn real_shell_streams_stdout_then_stderr() {
        let mut lines = Vec::new();
        let ok = RealShell
            .run_streaming(
                "sh",
                &["-c", "echo a; echo b >&2; echo c; exit 3"],
                &mut |l| lines.push(l.to_string()),
            )
            .unwrap();
        assert!(!ok);
        assert_eq!(lines, ["a", "c", "b"]);
    }

    #[test]
    fn output_that_is_not_utf8_keeps_streaming() {
        let mut lines = Vec::new();
        let ok = RealShell
            .run_streaming(
                "sh",
                &[
                    "-c",
                    r"printf 'a\377b\r\nnext\n'; printf 'e\376\n' >&2; printf tail",
                ],
                &mut |l| lines.push(l.to_string()),
            )
            .unwrap();
        assert!(ok);
        assert_eq!(lines, ["a\u{FFFD}b", "next", "tail", "e\u{FFFD}"]);
    }
}
