//! The Spotify login dialog of Configure: show the authorize URL (as text
//! and as a half-block QR code), take the address the browser ends up on,
//! exchange its code for tokens on a worker thread and write the token file
//! for the service user.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use rackscreen_sources::spotify::{
    apply_token_response, code_form, new_login, parse_display_name, parse_redirect, write_token,
    Login, TokenFile, ME_URL, REDIRECT_URI, TOKEN_URL,
};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::theme::Theme;

/// `(client id, code, verifier)` to a token file with the display name set.
pub type Exchange = Arc<dyn Fn(&str, &str, &str) -> Result<TokenFile> + Send + Sync>;

/// Where the token file goes and who owns it.
pub type Target = (PathBuf, Option<(u32, u32)>);

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The real exchange: the token endpoint, then `/v1/me` for the name.
pub fn real_exchange() -> Exchange {
    Arc::new(|client_id: &str, code: &str, verifier: &str| {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent(concat!("rackscreen/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("build http client")?;
        let resp = http
            .post(TOKEN_URL)
            .form(&code_form(client_id, code, verifier))
            .send()
            .context("reach accounts.spotify.com")?;
        let status = resp.status().as_u16();
        let body = resp.text().unwrap_or_default();
        let mut tok = apply_token_response(status, &body, None, unix_now())
            .map_err(|e| anyhow::anyhow!("Spotify refused the code: {e}"))?;
        if let Ok(me) = http.get(ME_URL).bearer_auth(&tok.access_token).send() {
            tok.display_name = parse_display_name(&me.text().unwrap_or_default());
        }
        Ok(tok)
    })
}

/// A QR code of `text` in half blocks, light modules drawn (for a dark
/// terminal), with a two-module quiet zone. Empty if it does not encode.
pub fn qr_lines(text: &str) -> Vec<String> {
    let Ok(code) = qrcode::QrCode::with_error_correction_level(text, qrcode::EcLevel::L) else {
        return Vec::new();
    };
    let w = code.width();
    let colors = code.to_colors();
    const Q: usize = 2;
    let n = w + 2 * Q;
    let light = |x: usize, y: usize| -> bool {
        if x < Q || y < Q || x >= w + Q || y >= w + Q {
            return true;
        }
        colors[(y - Q) * w + (x - Q)] == qrcode::Color::Light
    };
    (0..n.div_ceil(2))
        .map(|row| {
            (0..n)
                .map(|x| {
                    let top = light(x, row * 2);
                    let bottom = row * 2 + 1 >= n || light(x, row * 2 + 1);
                    match (top, bottom) {
                        (true, true) => '█',
                        (true, false) => '▀',
                        (false, true) => '▄',
                        (false, false) => ' ',
                    }
                })
                .collect()
        })
        .collect()
}

/// One login attempt in progress.
pub struct LoginFlow {
    client_id: String,
    login: Login,
    pub buf: String,
    pub error: Option<String>,
    rx: Option<Receiver<Result<TokenFile, String>>>,
}

impl LoginFlow {
    pub fn start(client_id: &str) -> Result<LoginFlow> {
        Ok(Self::with_login(client_id, new_login(client_id)?))
    }

    pub fn with_login(client_id: &str, login: Login) -> LoginFlow {
        LoginFlow {
            client_id: client_id.trim().to_string(),
            login,
            buf: String::new(),
            error: None,
            rx: None,
        }
    }

    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    pub fn url(&self) -> &str {
        &self.login.url
    }

    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }

    /// Check the pasted address and start the exchange on a thread.
    pub fn submit(&mut self, exchange: Exchange, target: Target) {
        let code = match parse_redirect(&self.buf, &self.login.state) {
            Ok(c) => c,
            Err(e) => {
                self.error = Some(e.to_string());
                return;
            }
        };
        self.error = None;
        let (tx, rx) = mpsc::channel();
        let (client_id, verifier) = (self.client_id.clone(), self.login.verifier.clone());
        std::thread::Builder::new()
            .name("spotify-login".into())
            .spawn(move || {
                let res = exchange(&client_id, &code, &verifier)
                    .and_then(|tok| {
                        write_token(&target.0, &tok, target.1)?;
                        Ok(tok)
                    })
                    .map_err(|e| format!("{e:#}"));
                let _ = tx.send(res);
            })
            .expect("spawn spotify login");
        self.rx = Some(rx);
    }

    /// The exchange's outcome once it is in: the token file written, or
    /// why not (the dialog then stays open with the error).
    pub fn poll(&mut self) -> Option<Result<TokenFile, String>> {
        let res = match self.rx.as_ref()?.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err("login thread died".into()),
        };
        self.rx = None;
        if let Err(e) = &res {
            self.error = Some(e.clone());
        }
        Some(res)
    }

    pub fn draw(&self, f: &mut Frame, area: Rect, th: &Theme) {
        f.render_widget(Clear, area);
        let block = Block::bordered()
            .border_style(th.selected())
            .title(Line::from(Span::styled(" Spotify login ", th.selected())));
        let inner = block.inner(area);
        f.render_widget(block, area);
        let mut lines = vec![
            Line::from(Span::styled(
                "1. Open this address on any device, or scan the code:",
                th.normal(),
            )),
            Line::from(Span::styled(self.login.url.clone(), th.title())),
        ];
        let qr = qr_lines(&self.login.url);
        let text_rows = 9;
        if !qr.is_empty()
            && (qr.len() + text_rows) as u16 <= inner.height
            && qr[0].chars().count() as u16 <= inner.width
        {
            lines.push(Line::from(""));
            lines.extend(qr.into_iter().map(|l| Line::from(Span::raw(l))));
        } else {
            lines.push(Line::from(Span::styled(
                "(make the terminal bigger for a QR code)",
                th.muted(),
            )));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("2. Log in. The browser then fails to open {REDIRECT_URI}; that is fine."),
            th.normal(),
        )));
        lines.push(Line::from(Span::styled(
            "3. Paste the whole address from the address bar here:",
            th.normal(),
        )));
        lines.push(Line::from(Span::styled(
            format!("> {}_", self.buf),
            th.title(),
        )));
        if !self.buf.is_empty() {
            let has_state = self.buf.contains("state=");
            lines.push(Line::from(Span::styled(
                format!(
                    "{} characters{}",
                    self.buf.chars().count(),
                    if has_state {
                        ""
                    } else {
                        " (no state= part: fine, the code is enough)"
                    }
                ),
                th.muted(),
            )));
        }
        if self.busy() {
            lines.push(Line::from(Span::styled("logging in...", th.warning())));
        } else if let Some(e) = &self.error {
            lines.push(Line::from(Span::styled(e.clone(), th.bad())));
        }
        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rackscreen_sources::spotify::{login_from, read_token};

    fn flow() -> LoginFlow {
        LoginFlow::with_login("cid", login_from("cid", &[1; 32], &[2; 12]))
    }

    fn fake_exchange() -> Exchange {
        Arc::new(|client_id: &str, code: &str, verifier: &str| {
            anyhow::ensure!(client_id == "cid" && code == "the-code", "bad request");
            anyhow::ensure!(verifier.len() == 43, "verifier");
            Ok(TokenFile {
                access_token: "a".into(),
                refresh_token: "r".into(),
                expires_at: 1,
                display_name: Some("Silke".into()),
                error: None,
            })
        })
    }

    fn wait(f: &mut LoginFlow) -> Result<TokenFile, String> {
        for _ in 0..200 {
            if let Some(r) = f.poll() {
                return r;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("no result")
    }

    #[test]
    fn qr_is_square_half_blocks_with_a_quiet_zone() {
        let lines = qr_lines("https://accounts.spotify.com/authorize?client_id=abc");
        let w = lines[0].chars().count();
        assert_eq!(lines.len(), w.div_ceil(2), "two modules per row");
        assert!(lines.iter().all(|l| l.chars().count() == w));
        assert!(lines[0].chars().all(|c| c == '█'), "quiet zone on top");
        assert!(lines.iter().all(|l| l.starts_with("██")), "quiet zone left");
        assert!(lines.iter().any(|l| l.contains(' ')), "dark modules");
    }

    #[test]
    fn a_pasted_address_is_exchanged_and_written_for_the_user() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state/rackscreen/spotify.json");
        let mut f = flow();
        assert!(f.url().contains("client_id=cid"));
        f.buf = "http://127.0.0.1:8888/callback?code=the-code&state=wrong".into();
        f.submit(fake_exchange(), (path.clone(), None));
        assert!(!f.busy());
        assert!(f
            .error
            .as_deref()
            .unwrap()
            .contains("another login attempt"));
        let state = url::Url::parse(f.url())
            .unwrap()
            .query_pairs()
            .find(|(k, _)| k == "state")
            .unwrap()
            .1
            .into_owned();
        f.buf = format!("http://127.0.0.1:8888/callback?code=the-code&state={state}");
        f.submit(fake_exchange(), (path.clone(), None));
        assert!(f.busy() && f.error.is_none());
        let tok = wait(&mut f).unwrap();
        assert_eq!(tok.display_name.as_deref(), Some("Silke"));
        assert_eq!(read_token(&path), Some(tok));
        assert!(!f.busy());
    }

    #[test]
    fn a_failed_exchange_keeps_the_dialog_with_the_error() {
        let dir = tempfile::tempdir().unwrap();
        let mut f = flow();
        let state = url::Url::parse(f.url())
            .unwrap()
            .query_pairs()
            .find(|(k, _)| k == "state")
            .unwrap()
            .1
            .into_owned();
        f.buf = format!("code=other-code&state={state}");
        f.submit(fake_exchange(), (dir.path().join("t.json"), None));
        assert_eq!(wait(&mut f), Err("bad request".to_string()));
        assert_eq!(f.error.as_deref(), Some("bad request"));
        assert!(!dir.path().join("t.json").exists());
    }
}
