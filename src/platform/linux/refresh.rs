use super::on_path;
use std::env;
use std::ffi::OsStr;
use std::io::{self, BufRead, IsTerminal, Write};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const THUNAR: &str = "org.xfce.Thunar";
const XFDESKTOP: &str = "org.xfce.xfdesktop";

pub(super) fn offer_xfce_refresh(search_path: Option<&OsStr>) {
    let graphical = ["DISPLAY", "WAYLAND_DISPLAY"]
        .iter()
        .any(|var| env::var_os(var).is_some_and(|value| !value.is_empty()));
    if !graphical || !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return;
    }

    if on_path(search_path, "thunar")
        && bus_owner(THUNAR).is_some()
        && confirm("Run `thunar --quit` now? This closes all Thunar windows.")
    {
        match quit("thunar") {
            Ok(()) => println!("Reopen Thunar to load the updated menu."),
            Err(err) => eprintln!("Could not quit Thunar: {err}"),
        }
    }

    if on_path(search_path, "xfdesktop") {
        if let Some(owner) = bus_owner(XFDESKTOP) {
            if confirm("Restart xfdesktop now? Desktop icons may briefly disappear.") {
                if let Err(err) = restart_xfdesktop(&owner) {
                    eprintln!(
                        "Could not restart xfdesktop: {err}. You can sign out and back in instead."
                    );
                }
            }
        }
    }
}

fn bus_owner(name: &str) -> Option<Vec<u8>> {
    // Query the session bus without starting a desktop application that isn't running.
    let output = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--timeout",
            "1",
            "--dest",
            "org.freedesktop.DBus",
            "--object-path",
            "/org/freedesktop/DBus",
            "--method",
            "org.freedesktop.DBus.GetNameOwner",
            name,
        ])
        .stdin(Stdio::null())
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

fn confirm(question: &str) -> bool {
    ask(question, io::stdin().lock(), io::stdout().lock()).unwrap_or(false)
}

fn ask(question: &str, mut input: impl BufRead, mut output: impl Write) -> io::Result<bool> {
    write!(output, "{question} [Y/n] ")?;
    output.flush()?;
    let mut answer = String::new();
    if input.read_line(&mut answer)? == 0 {
        return Ok(false);
    }
    let answer = answer.trim();
    Ok(answer.is_empty() || answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes"))
}

fn quit(program: &str) -> io::Result<()> {
    let status = Command::new(program).arg("--quit").status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("{program} --quit exited with {status}")))
    }
}

fn restart_xfdesktop(previous_owner: &[u8]) -> io::Result<()> {
    quit("xfdesktop")?;
    // Don't invoke a new copy until the old instance releases its name. A session
    // manager may already have started a replacement for us.
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match bus_owner(XFDESKTOP) {
            Some(owner) if owner != previous_owner => return Ok(()),
            Some(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
            Some(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "xfdesktop did not exit",
                ));
            }
            None => break,
        }
    }
    Command::new("xfdesktop")
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ask;

    #[test]
    fn enter_defaults_to_yes_but_eof_does_not_confirm() {
        for (reply, expected) in [
            ("y\n", true),
            ("YES\n", true),
            ("N\n", false),
            ("\n", true),
            ("", false),
            ("maybe\n", false),
        ] {
            let mut output = Vec::new();
            assert_eq!(
                ask("Restart?", reply.as_bytes(), &mut output).unwrap(),
                expected
            );
            assert_eq!(output, b"Restart? [Y/n] ");
        }
    }
}
