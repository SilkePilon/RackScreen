//! Pure edits to /boot/firmware/config.txt and cmdline.txt.

pub const CONFIG_LINES: [&str; 2] = ["dtparam=spi=on", "dtoverlay=spi1-2cs"];
pub const CMDLINE_TOKEN: &str = "spidev.bufsiz=65536";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootChange {
    ConfigLine(&'static str),
    CmdlineToken(&'static str),
}

impl BootChange {
    pub fn describe(&self) -> String {
        match self {
            BootChange::ConfigLine(l) => format!("config.txt  + {l}"),
            BootChange::CmdlineToken(t) => format!("cmdline.txt + {t}"),
        }
    }
}

fn has_line(text: &str, line: &str) -> bool {
    text.lines()
        .map(|l| l.trim().trim_end_matches('\r'))
        .any(|l| l == line)
}

/// Append `line` if no non-comment line equals it. Returns (new text, changed).
pub fn ensure_line(text: &str, line: &str) -> (String, bool) {
    if has_line(text, line) {
        return (text.to_string(), false);
    }
    let mut out = text.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(line);
    out.push('\n');
    (out, true)
}

/// cmdline.txt is one line of space separated tokens. Append `token` if missing.
pub fn ensure_cmdline_token(text: &str, token: &str) -> (String, bool) {
    let line = text.lines().next().unwrap_or("").trim_end_matches('\r');
    if line.split_whitespace().any(|t| t == token) {
        return (text.to_string(), false);
    }
    let mut out = line.trim_end().to_string();
    if !out.is_empty() {
        out.push(' ');
    }
    out.push_str(token);
    out.push('\n');
    (out, true)
}

pub fn needed_changes(config_txt: &str, cmdline: &str) -> Vec<BootChange> {
    let mut v = Vec::new();
    for l in CONFIG_LINES {
        if !has_line(config_txt, l) {
            v.push(BootChange::ConfigLine(l));
        }
    }
    if !ensure_cmdline_token(cmdline, CMDLINE_TOKEN).1 {
        // token present
    } else {
        v.push(BootChange::CmdlineToken(CMDLINE_TOKEN));
    }
    v
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Readiness {
    pub spi_on: bool,
    pub spi1_overlay: bool,
    pub bufsiz: bool,
}

pub fn readiness(config_txt: &str, cmdline: &str) -> Readiness {
    Readiness {
        spi_on: has_line(config_txt, CONFIG_LINES[0]),
        spi1_overlay: has_line(config_txt, CONFIG_LINES[1]),
        bufsiz: !ensure_cmdline_token(cmdline, CMDLINE_TOKEN).1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_line_cases() {
        assert_eq!(ensure_line("", "a=b"), ("a=b\n".into(), true));
        assert_eq!(
            ensure_line("x=1\na=b\n", "a=b"),
            ("x=1\na=b\n".into(), false)
        );
        assert_eq!(ensure_line("x=1", "a=b"), ("x=1\na=b\n".into(), true));
        assert!(
            !ensure_line("  a=b  \r\n", "a=b").1,
            "trailing spaces and CRLF still count"
        );
        assert!(
            ensure_line("#a=b\n", "a=b").1,
            "commented line does not count"
        );
    }

    #[test]
    fn cmdline_token_cases() {
        let base = "console=serial0,115200 root=PARTUUID=abc rootwait";
        let (t, changed) = ensure_cmdline_token(base, CMDLINE_TOKEN);
        assert!(changed);
        assert_eq!(t, format!("{base} {CMDLINE_TOKEN}\n"));
        assert!(!ensure_cmdline_token(&t, CMDLINE_TOKEN).1);
        assert_eq!(
            ensure_cmdline_token("", CMDLINE_TOKEN).0,
            format!("{CMDLINE_TOKEN}\n")
        );
    }

    #[test]
    fn needed_and_readiness() {
        let all = needed_changes("", "");
        assert_eq!(all.len(), 3);
        let none = needed_changes(
            "dtparam=spi=on\ndtoverlay=spi1-2cs\n",
            "root=x spidev.bufsiz=65536",
        );
        assert!(none.is_empty());
        let r = readiness("dtparam=spi=on\n", "");
        assert_eq!(
            r,
            Readiness {
                spi_on: true,
                spi1_overlay: false,
                bufsiz: false
            }
        );
        assert_eq!(
            BootChange::CmdlineToken(CMDLINE_TOKEN).describe(),
            "cmdline.txt + spidev.bufsiz=65536"
        );
    }
}
