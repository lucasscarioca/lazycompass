use anyhow::{Context, Result};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardMethod {
    Native,
    Osc52,
}

pub(crate) fn copy_to_clipboard(contents: &str) -> Result<()> {
    let methods = if prefers_osc52() {
        [ClipboardMethod::Osc52, ClipboardMethod::Native]
    } else {
        [ClipboardMethod::Native, ClipboardMethod::Osc52]
    };

    let mut errors = Vec::new();
    for method in methods {
        let result = match method {
            ClipboardMethod::Native => copy_with_native_command(contents),
            ClipboardMethod::Osc52 => copy_with_osc52(contents),
        };
        match result {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error.to_string()),
        }
    }

    anyhow::bail!("clipboard copy failed: {}", errors.join("; "))
}

fn prefers_osc52() -> bool {
    std::env::var_os("SSH_TTY").is_some()
}

fn copy_with_native_command(contents: &str) -> Result<()> {
    let commands = native_commands();
    let mut errors = Vec::new();
    for (program, args) in commands {
        match run_native_command(program, args, contents) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error.to_string()),
        }
    }
    anyhow::bail!("no supported clipboard command: {}", errors.join("; "))
}

#[cfg(target_os = "macos")]
fn native_commands() -> Vec<(&'static str, &'static [&'static str])> {
    vec![("pbcopy", &[])]
}

#[cfg(not(target_os = "macos"))]
fn native_commands() -> Vec<(&'static str, &'static [&'static str])> {
    vec![
        ("wl-copy", &[]),
        ("xclip", &["-selection", "clipboard"]),
        ("xsel", &["--clipboard", "--input"]),
    ]
}

fn run_native_command(program: &str, args: &[&str], contents: &str) -> Result<()> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .spawn()
        .with_context(|| format!("unable to launch {program}"))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("{program} stdin unavailable"))?;
    stdin
        .write_all(contents.as_bytes())
        .with_context(|| format!("unable to write to {program} stdin"))?;
    drop(stdin);
    let status = child
        .wait()
        .with_context(|| format!("unable to wait for {program}"))?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("{program} exited with non-zero status");
    }
}

fn copy_with_osc52(contents: &str) -> Result<()> {
    let sequence = osc52_sequence(contents);
    let mut stdout = std::io::stdout();
    stdout
        .write_all(sequence.as_bytes())
        .context("unable to write OSC52 clipboard sequence")?;
    stdout
        .flush()
        .context("unable to flush OSC52 clipboard sequence")?;
    Ok(())
}

pub(crate) fn osc52_sequence(contents: &str) -> String {
    format!("\u{1b}]52;c;{}\u{7}", STANDARD.encode(contents.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::{osc52_sequence, prefers_osc52};

    #[test]
    fn osc52_sequence_wraps_base64_payload() {
        assert_eq!(osc52_sequence("hi"), "\u{1b}]52;c;aGk=\u{7}");
    }

    #[test]
    fn prefers_osc52_follows_ssh_tty() {
        let original = std::env::var_os("SSH_TTY");
        unsafe {
            std::env::remove_var("SSH_TTY");
        }
        assert!(!prefers_osc52());

        unsafe {
            std::env::set_var("SSH_TTY", "/tmp/tty");
        }
        assert!(prefers_osc52());

        match original {
            Some(value) => unsafe {
                std::env::set_var("SSH_TTY", value);
            },
            None => unsafe {
                std::env::remove_var("SSH_TTY");
            },
        }
    }
}
