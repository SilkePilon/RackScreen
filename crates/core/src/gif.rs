//! Decoded GIFs for the `gif:<name>` role: the frames, their timing, the
//! link patterns the role accepts and the frame cap.

use std::sync::Arc;

use crate::anim::Secs;

/// Frames are square, the panel's size.
pub const GIF_SIDE: u32 = 240;
/// Bytes in one frame: premultiplied RGBA, `GIF_SIDE`².
pub const FRAME_BYTES: usize = (GIF_SIDE * GIF_SIDE * 4) as usize;
/// A longer GIF keeps every n-th frame; 150 frames are about 35 MB.
pub const MAX_FRAMES: usize = 150;

/// A GIF frame delay in centiseconds as milliseconds. 0 and 1 cs (0 and
/// 10 ms) play at 100 ms, as browsers do.
pub fn delay_ms(centis: u16) -> u32 {
    if centis <= 1 {
        100
    } else {
        centis as u32 * 10
    }
}

/// Which of `n` frames survive the cap of `max`: every `ceil(n / max)`-th,
/// starting with the first.
pub fn keep_mask(n: usize, max: usize) -> Vec<bool> {
    let step = n.div_ceil(max.max(1)).max(1);
    (0..n).map(|i| i % step == 0).collect()
}

/// Apply `keep_mask` to `(frame, delay)` pairs: a skipped frame's delay is
/// added to the kept frame before it, so the loop keeps its length.
pub fn cap_frames<T>(frames: Vec<(T, u32)>, max: usize) -> Vec<(T, u32)> {
    let mask = keep_mask(frames.len(), max);
    let mut out: Vec<(T, u32)> = Vec::with_capacity(frames.len().min(max));
    for ((f, d), keep) in frames.into_iter().zip(mask) {
        match out.last_mut() {
            Some(last) if !keep => last.1 += d,
            _ => out.push((f, d)),
        }
    }
    out
}

/// The decoded frames of one GIF, ready to blit.
#[derive(Debug, PartialEq)]
pub struct GifFrames {
    frames: Vec<Vec<u8>>,
    delays_ms: Vec<u32>,
    total_ms: u64,
}

impl GifFrames {
    /// Frames of `FRAME_BYTES` each with their delay; at least one.
    pub fn new(frames: Vec<(Vec<u8>, u32)>) -> Result<GifFrames, String> {
        if frames.is_empty() {
            return Err("no frames".into());
        }
        if let Some((f, _)) = frames.iter().find(|(f, _)| f.len() != FRAME_BYTES) {
            return Err(format!("frame of {} bytes, want {FRAME_BYTES}", f.len()));
        }
        let (frames, delays_ms): (Vec<Vec<u8>>, Vec<u32>) = frames.into_iter().unzip();
        let total_ms = delays_ms.iter().map(|d| *d as u64).sum();
        Ok(GifFrames {
            frames,
            delays_ms,
            total_ms,
        })
    }
    pub fn len(&self) -> usize {
        self.frames.len()
    }
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
    pub fn frame(&self, i: usize) -> &[u8] {
        &self.frames[i]
    }
    pub fn delay_ms(&self, i: usize) -> u32 {
        self.delays_ms[i]
    }
    /// One loop, in milliseconds.
    pub fn total_ms(&self) -> u64 {
        self.total_ms
    }
    /// The frame on screen `elapsed` seconds after the first one, looping.
    pub fn frame_at(&self, elapsed: Secs) -> usize {
        if self.total_ms == 0 || elapsed <= 0.0 {
            return 0;
        }
        let mut t = ((elapsed * 1000.0).round() as u64) % self.total_ms;
        for (i, d) in self.delays_ms.iter().enumerate() {
            let d = *d as u64;
            if t < d {
                return i;
            }
            t -= d;
        }
        self.frames.len() - 1
    }
}

/// A shared decoded GIF, compared by identity like `WorldRef`: one `Arc` per
/// GIF, so two scenes showing the same frame of it are equal.
#[derive(Clone)]
pub struct GifRef(pub Arc<GifFrames>);

impl PartialEq for GifRef {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for GifRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "GifRef({} frames @ {:p})",
            self.0.len(),
            Arc::as_ptr(&self.0)
        )
    }
}

/// Where one configured GIF stands, as the model sees it.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum GifState {
    /// Not decoded yet (downloading, or nothing arrived).
    #[default]
    Loading,
    Ready(GifRef),
    /// The last attempt failed; the source retries.
    Failed,
}

/// What a pasted link needs before its `.gif` can be downloaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GifLink {
    /// A `.gif` URL to download as is.
    Direct(String),
    /// A Tenor page: fetch it and read the `.gif` URL out of the HTML.
    TenorPage(String),
}

/// Sort a pasted link: GIPHY page or media links become `i.giphy.com/<id>.gif`,
/// Tenor pages need a fetch, `media*.tenor.com` and any other https `.gif`
/// link is used as is.
pub fn classify_link(url: &str) -> Result<GifLink, String> {
    let url = url.trim();
    let rest = url
        .strip_prefix("https://")
        .ok_or("the link must start with https://")?;
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    if let Some(why) = host_refusal(host) {
        return Err(why);
    }
    let host = host.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    let path = path.split(['?', '#']).next().unwrap_or("");
    let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let is_gif = path.to_ascii_lowercase().ends_with(".gif");
    let giphy = |id: &str| -> Result<GifLink, String> {
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(format!("no GIPHY id in {url}"));
        }
        Ok(GifLink::Direct(format!("https://i.giphy.com/{id}.gif")))
    };
    if host == "giphy.com" {
        return match segs.as_slice() {
            ["gifs", slug, ..] => giphy(slug.rsplit('-').next().unwrap_or(slug)),
            _ => Err("a GIPHY link must be a giphy.com/gifs/... page".into()),
        };
    }
    if host == "i.giphy.com" {
        return match segs.as_slice() {
            [file] => giphy(file.split('.').next().unwrap_or(file)),
            _ => Err(format!("no GIPHY id in {url}")),
        };
    }
    if host.ends_with(".giphy.com") && host.starts_with("media") {
        return match segs.as_slice() {
            ["media", .., id, _file] => giphy(id),
            _ => Err(format!("no GIPHY id in {url}")),
        };
    }
    if host == "tenor.com" {
        return if segs.iter().take(2).any(|s| *s == "view") {
            Ok(GifLink::TenorPage(url.to_string()))
        } else {
            Err("a Tenor link must be a tenor.com/view/... page".into())
        };
    }
    if is_gif {
        return Ok(GifLink::Direct(url.to_string()));
    }
    Err("not a GIPHY or Tenor link, or an https link ending in .gif".into())
}

/// An IP a GIF must never come from: loopback, private, link-local,
/// carrier-grade NAT, unspecified or broadcast (an IPv4-mapped IPv6 address
/// counts as its IPv4 one). GIFs come from the internet.
pub fn refused_ip(ip: std::net::IpAddr) -> bool {
    use std::net::IpAddr;
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || (a == 100 && (64..128).contains(&b))
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return refused_ip(IpAddr::V4(v4));
            }
            let first = v6.segments()[0];
            v6.is_loopback()
                || v6.is_unspecified()
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
        }
    }
}

/// Why a GIF may not be fetched from `host` (as it appears in a URL, maybe
/// with `user@` and `:port`), or `None` when it may: an IP literal that
/// `refused_ip`, `localhost` or an empty host.
pub fn host_refusal(host: &str) -> Option<String> {
    let host = host.rsplit('@').next().unwrap_or(host);
    let bare = match host.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or(v6),
        None => host.split(':').next().unwrap_or(host),
    };
    let lower = bare.to_ascii_lowercase();
    let lower = lower.trim_end_matches('.');
    if lower.is_empty() {
        return Some("the link has no host".into());
    }
    if lower == "localhost" || lower.ends_with(".localhost") {
        return Some(format!(
            "{bare} is this machine; GIFs must come from the internet"
        ));
    }
    match lower.parse::<std::net::IpAddr>() {
        Ok(ip) if refused_ip(ip) => Some(format!(
            "{bare} is a local or private address; GIFs must come from the internet"
        )),
        _ => None,
    }
}

/// GIF names: lowercase letters, digits and `-`, not empty.
pub fn valid_gif_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(v: u8) -> Vec<u8> {
        vec![v; FRAME_BYTES]
    }

    #[test]
    fn zero_and_ten_ms_delays_play_at_100_ms() {
        assert_eq!(delay_ms(0), 100);
        assert_eq!(delay_ms(1), 100);
        assert_eq!(delay_ms(2), 20);
        assert_eq!(delay_ms(7), 70);
        assert_eq!(delay_ms(100), 1000);
    }

    #[test]
    fn short_gifs_keep_every_frame() {
        assert_eq!(keep_mask(3, 150), vec![true; 3]);
        assert_eq!(keep_mask(150, 150), vec![true; 150]);
        let f = vec![('a', 10), ('b', 20)];
        assert_eq!(cap_frames(f.clone(), 150), f);
    }

    #[test]
    fn long_gifs_keep_every_nth_frame_and_their_timing() {
        let m = keep_mask(301, 150);
        assert_eq!(m.iter().filter(|k| **k).count(), 101, "every third frame");
        assert!(m[0] && !m[1] && !m[2] && m[3]);
        let frames: Vec<(usize, u32)> = (0..301).map(|i| (i, 10 + i as u32)).collect();
        let total: u32 = frames.iter().map(|f| f.1).sum();
        let kept = cap_frames(frames, 150);
        assert!(kept.len() <= 150);
        assert_eq!(
            kept[0],
            (0, 10 + 11 + 12),
            "skipped delays join the kept frame"
        );
        assert_eq!(kept[1].0, 3);
        assert_eq!(
            kept.iter().map(|f| f.1).sum::<u32>(),
            total,
            "loop length holds"
        );
        let m = keep_mask(151, 150);
        assert_eq!(m.iter().filter(|k| **k).count(), 76);
    }

    #[test]
    fn frames_must_be_panel_sized_and_present() {
        assert!(GifFrames::new(vec![]).is_err());
        assert!(GifFrames::new(vec![(vec![0; 16], 100)]).is_err());
        let g = GifFrames::new(vec![(frame(1), 100), (frame(2), 50)]).unwrap();
        assert_eq!((g.len(), g.total_ms()), (2, 150));
        assert_eq!(g.frame(1)[0], 2);
        assert_eq!(g.delay_ms(1), 50);
    }

    #[test]
    fn frame_follows_the_delays_and_loops() {
        let g = GifFrames::new(vec![(frame(0), 100), (frame(1), 50), (frame(2), 200)]).unwrap();
        assert_eq!(g.frame_at(0.0), 0);
        assert_eq!(g.frame_at(0.099), 0);
        assert_eq!(g.frame_at(0.100), 1);
        assert_eq!(g.frame_at(0.149), 1);
        assert_eq!(g.frame_at(0.150), 2);
        assert_eq!(g.frame_at(0.349), 2);
        assert_eq!(g.frame_at(0.350), 0, "loops");
        assert_eq!(g.frame_at(3.5 + 0.12), 1);
        assert_eq!(g.frame_at(-1.0), 0, "before the start");
        let one = GifFrames::new(vec![(frame(0), 100)]).unwrap();
        assert_eq!(one.frame_at(12.3), 0);
    }

    #[test]
    fn gif_ref_compares_by_identity() {
        let a = GifRef(Arc::new(GifFrames::new(vec![(frame(0), 100)]).unwrap()));
        let b = GifRef(Arc::new(GifFrames::new(vec![(frame(0), 100)]).unwrap()));
        assert_eq!(a, a.clone());
        assert_ne!(a, b, "equal pixels, different GIFs");
    }

    #[test]
    fn giphy_links_become_the_direct_gif() {
        let want = GifLink::Direct("https://i.giphy.com/AbCdEf123.gif".into());
        for url in [
            "https://giphy.com/gifs/some-slug-AbCdEf123",
            "https://giphy.com/gifs/AbCdEf123",
            "https://giphy.com/gifs/some-slug-AbCdEf123/",
            "https://www.giphy.com/gifs/cat-dance-AbCdEf123?utm=x",
            "https://media.giphy.com/media/AbCdEf123/giphy.gif",
            "https://media4.giphy.com/media/v1.Y2lk/AbCdEf123/giphy.webp",
            "https://media0.giphy.com/media/AbCdEf123/200.gif?cid=1",
            "  https://giphy.com/gifs/some-slug-AbCdEf123  ",
        ] {
            assert_eq!(classify_link(url), Ok(want.clone()), "{url}");
        }
        assert_eq!(
            classify_link("https://i.giphy.com/AbCdEf123.gif"),
            Ok(want.clone())
        );
    }

    #[test]
    fn tenor_pages_are_fetched_and_media_links_used_as_is() {
        assert_eq!(
            classify_link("https://tenor.com/view/some-slug-12345678"),
            Ok(GifLink::TenorPage(
                "https://tenor.com/view/some-slug-12345678".into()
            ))
        );
        assert_eq!(
            classify_link("https://tenor.com/nl/view/some-slug-12345678"),
            Ok(GifLink::TenorPage(
                "https://tenor.com/nl/view/some-slug-12345678".into()
            ))
        );
        let media = "https://media1.tenor.com/m/abcDEF/some-slug.gif";
        assert_eq!(classify_link(media), Ok(GifLink::Direct(media.into())));
    }

    #[test]
    fn other_https_gif_links_are_used_as_is_and_the_rest_refused() {
        let ok = "https://example.com/a/b/party.gif";
        assert_eq!(classify_link(ok), Ok(GifLink::Direct(ok.into())));
        let q = "https://example.com/party.GIF?x=1";
        assert_eq!(classify_link(q), Ok(GifLink::Direct(q.into())));
        for bad in [
            "",
            "http://example.com/party.gif",
            "https://example.com/party.png",
            "https://giphy.com/explore/cats",
            "https://tenor.com/search/cats",
            "giphy.com/gifs/AbCdEf123",
        ] {
            assert!(classify_link(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn private_loopback_and_link_local_ips_are_refused() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.5",
            "169.254.1.1",
            "0.0.0.0",
            "100.64.0.1",
            "255.255.255.255",
            "::1",
            "::",
            "fe80::1",
            "fd00::1",
            "::ffff:192.168.1.5",
        ] {
            assert!(refused_ip(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["1.1.1.1", "8.8.8.8", "2606:4700::1111", "172.32.0.1"] {
            assert!(!refused_ip(ip.parse().unwrap()), "{ip}");
        }
    }

    #[test]
    fn hosts_are_checked_without_port_or_user() {
        assert!(host_refusal("giphy.com").is_none());
        assert!(host_refusal("media.tenor.com:443").is_none());
        assert!(host_refusal("8.8.8.8").is_none());
        for h in [
            "192.168.1.5",
            "192.168.1.5:8443",
            "u:p@10.0.0.1",
            "[::1]",
            "[fe80::1]:443",
            "localhost",
            "LOCALHOST:80",
            "x.localhost",
            "",
        ] {
            let e = host_refusal(h).unwrap_or_else(|| panic!("{h} allowed"));
            assert!(!e.is_empty(), "{h}");
        }
    }

    #[test]
    fn links_to_private_hosts_are_refused() {
        for url in [
            "https://192.168.1.5/x.gif",
            "https://127.0.0.1:8443/x.gif",
            "https://[::1]/x.gif",
            "https://localhost/x.gif",
        ] {
            let e = classify_link(url).unwrap_err();
            assert!(e.contains("internet"), "{url}: {e}");
        }
        assert!(classify_link("https://8.8.8.8/x.gif").is_ok());
    }

    #[test]
    fn gif_names_are_lowercase_slugs() {
        for ok in ["party", "nod-2", "a"] {
            assert!(valid_gif_name(ok), "{ok}");
        }
        for bad in ["", "Party", "my gif", "a_b", "gif:x", "é"] {
            assert!(!valid_gif_name(bad), "{bad}");
        }
    }
}
