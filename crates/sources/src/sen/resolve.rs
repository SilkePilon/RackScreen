//! From Sen's YouTube channel to HLS segment URLs, without a JS player:
//! the channel's live page gives the video id and the visitor data, the
//! innertube player (visionOS client) gives the HLS manifest, the master
//! playlist gives the 360p variant and its media playlist the newest
//! segment. Everything here is parsing; the requests are in `fetch`.

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};

pub const LIVE_PAGE_URL: &str = "https://www.youtube.com/@Sen/live";
pub const PLAYER_URL: &str = "https://www.youtube.com/youtubei/v1/player?prettyPrint=false";
/// The page and the player are asked as this browser.
pub const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15";
pub const CLIENT_NAME_ID: &str = "101";
pub const CLIENT_VERSION: &str = "1.02";

const CANONICAL: &str = "<link rel=\"canonical\" href=\"https://www.youtube.com/watch?v=";
const VISITOR: &str = "\"VISITOR_DATA\":\"";
/// The picture height the pipeline is built for, and the most it takes.
/// The visitor data is some 550 characters.
const MAX_VALUE: usize = 4096;
const WANT_HEIGHT: u32 = 360;
const MAX_HEIGHT: u32 = 480;

/// The quoted value after `marker` in `page`, when its closing quote is
/// there too and it only holds characters from `allowed`.
fn value_after<'a>(page: &'a str, marker: &str, allowed: &str) -> Option<&'a str> {
    let rest = &page[page.find(marker)? + marker.len()..];
    let value = &rest[..rest.find('"')?];
    let ok = !value.is_empty()
        && value.len() <= MAX_VALUE
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || allowed.contains(c));
    ok.then_some(value)
}

/// What the channel's live page says: the video that is live now, and the
/// visitor data the player wants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LivePage {
    pub video_id: String,
    pub visitor: String,
}

/// Finds the two values in the live page as it downloads, keeping only the
/// end of what was read so far (the page is 1.4 MB, the values sit some
/// 800 KB in).
#[derive(Default)]
pub struct PageScan {
    video_id: Option<String>,
    visitor: Option<String>,
    tail: Vec<u8>,
}

/// Enough of the page's end to hold a marker and its whole value.
const SCAN_KEEP: usize = MAX_VALUE + 256;

impl PageScan {
    /// Take the next piece of the page in; both values once they are found.
    pub fn push(&mut self, chunk: &[u8]) -> Option<LivePage> {
        self.tail.extend_from_slice(chunk);
        let text = String::from_utf8_lossy(&self.tail);
        if self.video_id.is_none() {
            self.video_id = value_after(&text, CANONICAL, "_-").map(str::to_string);
        }
        if self.visitor.is_none() {
            self.visitor = value_after(&text, VISITOR, "%_-=.").map(str::to_string);
        }
        if self.tail.len() > SCAN_KEEP {
            self.tail.drain(..self.tail.len() - SCAN_KEEP);
        }
        Some(LivePage {
            video_id: self.video_id.clone()?,
            visitor: self.visitor.clone()?,
        })
    }
}

/// Both values out of a whole page; `None` when one is missing.
pub fn parse_live_page(page: &str) -> Option<LivePage> {
    PageScan::default().push(page.as_bytes())
}

/// The innertube player request for `page`'s video.
pub fn player_request(page: &LivePage) -> Value {
    json!({
        "context": { "client": {
            "clientName": "VISIONOS",
            "clientVersion": CLIENT_VERSION,
            "deviceMake": "Apple",
            "deviceModel": "RealityDevice17,1",
            "osName": "visionOS",
            "osVersion": "26.5.23O471",
            "hl": "en",
            "visitorData": page.visitor,
        }},
        "videoId": page.video_id,
        "contentCheckOk": true,
        "racyCheckOk": true,
    })
}

/// The HLS manifest URL out of a player response.
pub fn parse_player(body: &[u8]) -> Result<String> {
    let v: Value = serde_json::from_slice(body).map_err(|e| anyhow!("player response: {e}"))?;
    let status = v["playabilityStatus"]["status"].as_str().unwrap_or("?");
    if status != "OK" {
        bail!("player says {status}");
    }
    let url = v["streamingData"]["hlsManifestUrl"]
        .as_str()
        .ok_or_else(|| anyhow!("no HLS manifest (the stream is not live)"))?;
    check_url(url)?;
    Ok(url.to_string())
}

/// Stream URLs are HTTPS on Google's video hosts; anything else in a
/// playlist is not followed.
pub fn check_url(url: &str) -> Result<()> {
    let parsed = url::Url::parse(url).map_err(|e| anyhow!("stream URL: {e}"))?;
    check_host(&parsed).map_err(|e| anyhow!(e))
}

/// The same rule for a parsed URL: HTTPS on YouTube or its video hosts.
/// Redirects are held to it as well.
pub fn check_host(url: &url::Url) -> Result<(), String> {
    let host = url.host_str().unwrap_or("");
    let ok = url.scheme() == "https"
        && (host == "googlevideo.com"
            || host.ends_with(".googlevideo.com")
            || host == "youtube.com"
            || host.ends_with(".youtube.com"));
    if ok {
        Ok(())
    } else {
        Err(format!("stream URL on an unexpected host: {host}"))
    }
}

/// Redirects the stream's requests may follow: a few, and only to hosts
/// `check_host` allows (the consent bounce stays on youtube.com).
pub const MAX_REDIRECTS: usize = 5;

pub fn check_redirect(to: &url::Url, hops_before: usize) -> Result<(), String> {
    if hops_before >= MAX_REDIRECTS {
        return Err("too many redirects".into());
    }
    check_host(to)
}

/// The media playlist of the variant to play: H.264 at 360p when there is
/// one, else the nearest height up to 480p.
pub fn pick_variant(master: &str) -> Result<String> {
    let mut best: Option<(u32, &str)> = None;
    let mut lines = master.lines();
    while let Some(line) = lines.next() {
        let Some(attrs) = line.strip_prefix("#EXT-X-STREAM-INF:") else {
            continue;
        };
        let Some(url) = lines.next().map(str::trim) else {
            break;
        };
        let height = attrs
            .split("RESOLUTION=")
            .nth(1)
            .and_then(|r| r.split(',').next())
            .and_then(|r| r.split('x').nth(1))
            .and_then(|h| h.parse::<u32>().ok());
        let Some(height) = height else {
            continue;
        };
        if !attrs.contains("avc1") || height > MAX_HEIGHT || url.starts_with('#') {
            continue;
        }
        // nearest to 360p, the sharper one of two as near; the first of
        // equals, since the list has one variant per audio track
        let rank = |h: u32| (h.abs_diff(WANT_HEIGHT), MAX_HEIGHT - h);
        if best.is_none_or(|(h, _)| rank(height) < rank(h)) {
            best = Some((height, url));
        }
    }
    let (_, url) = best.ok_or_else(|| anyhow!("no H.264 variant up to {MAX_HEIGHT}p"))?;
    check_url(url)?;
    Ok(url.to_string())
}

/// A segment URL split around its sequence number (`/sq/<n>/`). The stream's
/// segment URLs differ in nothing else, so the next ones follow from the
/// newest without reading the (large) playlist again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentUrl {
    prefix: String,
    pub sq: u64,
    suffix: String,
}

impl SegmentUrl {
    pub fn parse(url: &str) -> Result<SegmentUrl> {
        check_url(url)?;
        let at = url
            .find("/sq/")
            .ok_or_else(|| anyhow!("segment URL without a sequence number"))?;
        let digits = &url[at + 4..];
        let end = digits
            .find('/')
            .ok_or_else(|| anyhow!("segment URL ends in its sequence number"))?;
        let sq = digits[..end]
            .parse()
            .map_err(|_| anyhow!("segment sequence number is not a number"))?;
        Ok(SegmentUrl {
            prefix: url[..at + 4].to_string(),
            sq,
            suffix: digits[end..].to_string(),
        })
    }

    /// The URL of segment `sq`.
    pub fn url(&self, sq: u64) -> String {
        format!("{}{}{}", self.prefix, sq, self.suffix)
    }
}

/// The newest segment of a media playlist, from the playlist's end (the
/// whole text is not needed). An ended stream is an error.
pub fn newest_segment(playlist_tail: &str) -> Result<SegmentUrl> {
    if playlist_tail.contains("#EXT-X-ENDLIST") {
        bail!("the stream has ended");
    }
    let url = playlist_tail
        .lines()
        .rev()
        .map(str::trim)
        .find(|l| l.starts_with("https://"))
        .ok_or_else(|| anyhow!("no segment in the media playlist"))?;
    SegmentUrl::parse(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = include_str!("../../tests/fixtures/sen_live_page.html");
    const PLAYER: &[u8] = include_bytes!("../../tests/fixtures/sen_player.json");
    const MASTER: &str = include_str!("../../tests/fixtures/sen_master.m3u8");
    const MEDIA: &str = include_str!("../../tests/fixtures/sen_media.m3u8");

    #[test]
    fn the_live_page_gives_the_video_and_the_visitor() {
        let page = parse_live_page(PAGE).unwrap();
        assert_eq!(page.video_id, "fO9e9jnhYK8");
        assert_eq!(page.visitor, "CgtFWEFNUExFX1ZEMSiAgICAgICAgICAAQ%3D%3D");
        // a page cut before either value, or with odd characters in one
        let cut = PAGE.find("VISITOR_DATA").unwrap() + 20;
        assert_eq!(parse_live_page(&PAGE[..cut]), None);
        assert_eq!(parse_live_page(&PAGE[..40]), None);
        assert_eq!(parse_live_page(""), None);
        let odd = PAGE.replace("fO9e9jnhYK8", "fO9e9\\r\\nX: y");
        assert_eq!(parse_live_page(&odd), None);
        // the real visitor data is long
        let long = PAGE.replace("CgtFWEFNUExF", &"Cgt-_".repeat(110));
        assert_eq!(parse_live_page(&long).unwrap().visitor.len(), 578);
        // found the same in pieces of any size, with megabytes around them
        let big = format!("{}{PAGE}{}", "x".repeat(700_000), "y".repeat(50_000));
        let mut scan = PageScan::default();
        let found = PAGE.as_bytes().iter().find_map(|b| scan.push(&[*b]));
        assert_eq!(found, Some(page.clone()), "byte by byte");
        for size in [7, 1000, 16_384] {
            let mut scan = PageScan::default();
            let found = big.as_bytes().chunks(size).find_map(|c| scan.push(c));
            assert_eq!(found, Some(page.clone()), "pieces of {size}");
            assert!(scan.tail.len() <= SCAN_KEEP);
        }
        // a channel that is not live has no watch link
        let offline = PAGE.replace("watch?v=fO9e9jnhYK8", "@Sen");
        assert_eq!(parse_live_page(&offline), None);
    }

    #[test]
    fn the_player_request_names_the_video_and_the_visitor() {
        let page = parse_live_page(PAGE).unwrap();
        let body = player_request(&page);
        assert_eq!(body["videoId"], "fO9e9jnhYK8");
        assert_eq!(body["context"]["client"]["clientName"], "VISIONOS");
        assert_eq!(
            body["context"]["client"]["visitorData"],
            page.visitor.as_str()
        );
    }

    #[test]
    fn the_player_response_gives_the_manifest() {
        let url = parse_player(PLAYER).unwrap();
        assert!(url.starts_with("https://manifest.googlevideo.com/api/manifest/hls_variant/"));
        let err = |body: &str| parse_player(body.as_bytes()).unwrap_err().to_string();
        assert!(
            err(r#"{"playabilityStatus":{"status":"LOGIN_REQUIRED"}}"#).contains("LOGIN_REQUIRED")
        );
        assert!(
            err(r#"{"playabilityStatus":{"status":"OK"},"streamingData":{}}"#).contains("not live")
        );
        assert!(err("{").contains("player response"));
        assert!(err("[]").contains("?"));
        let elsewhere = r#"{"playabilityStatus":{"status":"OK"},"streamingData":{"hlsManifestUrl":"http://evil.example/x.m3u8"}}"#;
        assert!(err(elsewhere).contains("unexpected host"));
    }

    #[test]
    fn the_360p_variant_is_picked() {
        let url = pick_variant(MASTER).unwrap();
        assert!(url.contains("/itag/230/"), "{url}");
        // without it, the nearest height up to 480p
        let no_360: String = MASTER.replace("RESOLUTION=640x360", "RESOLUTION=1920x1080");
        assert!(
            pick_variant(&no_360).unwrap().contains("/itag/231/"),
            "480p"
        );
        let only_hd = "#EXTM3U\n#EXT-X-STREAM-INF:CODECS=\"avc1.640028\",RESOLUTION=1920x1080\nhttps://manifest.googlevideo.com/x\n";
        assert!(pick_variant(only_hd).is_err());
        let not_h264 = "#EXTM3U\n#EXT-X-STREAM-INF:CODECS=\"vp09.00\",RESOLUTION=640x360\nhttps://manifest.googlevideo.com/x\n";
        assert!(pick_variant(not_h264).is_err());
        let elsewhere =
            "#EXT-X-STREAM-INF:CODECS=\"avc1.4D401E\",RESOLUTION=640x360\nhttps://example.com/x\n";
        assert!(pick_variant(elsewhere).is_err());
        assert!(pick_variant("").is_err());
        assert!(pick_variant("#EXT-X-STREAM-INF:RESOLUTION=640x360").is_err());
    }

    #[test]
    fn the_newest_segment_comes_from_the_playlist_tail() {
        let seg = newest_segment(MEDIA).unwrap();
        assert_eq!(seg.sq, 4_294_237);
        let last = MEDIA
            .lines()
            .rev()
            .find(|l| l.starts_with("https"))
            .unwrap();
        assert_eq!(seg.url(seg.sq), last);
        // only the end of the playlist is needed
        let tail = &MEDIA[MEDIA.len() - 2000..];
        assert_eq!(newest_segment(tail).unwrap(), seg);
        // the next segment's URL differs in the number only
        let next = seg.url(seg.sq + 1);
        assert!(next.contains("/sq/4294238/"));
        assert_eq!(next.len(), last.len());
        assert_eq!(SegmentUrl::parse(&next).unwrap().sq, seg.sq + 1);
    }

    #[test]
    fn redirects_stay_on_the_stream_hosts() {
        let url = |s: &str| url::Url::parse(s).unwrap();
        let consent = url("https://consent.youtube.com/m?continue=https%3A%2F%2Fevil.example");
        assert!(check_redirect(&consent, 0).is_ok());
        assert!(check_redirect(&url("https://www.youtube.com/@Sen/live?cbrd=1"), 1).is_ok());
        assert!(check_redirect(&url("https://rr4---sn-x.googlevideo.com/a"), 2).is_ok());
        for bad in [
            "https://evil.example/x",
            "http://www.youtube.com/@Sen/live",
            "https://youtube.com.evil.example/",
            "https://notyoutube.com/",
            "https://192.168.1.1/",
            "https://localhost/",
        ] {
            assert!(check_redirect(&url(bad), 0).is_err(), "{bad}");
        }
        assert!(check_redirect(&consent, MAX_REDIRECTS).is_err(), "a loop");
    }

    #[test]
    fn odd_playlists_are_errors() {
        assert!(newest_segment("").is_err());
        assert!(newest_segment("#EXTM3U\n#EXT-X-TARGETDURATION:5\n").is_err());
        let ended = format!("{MEDIA}\n#EXT-X-ENDLIST\n");
        assert!(newest_segment(&ended)
            .unwrap_err()
            .to_string()
            .contains("ended"));
        assert!(SegmentUrl::parse("https://a.googlevideo.com/videoplayback/file/seg.ts").is_err());
        assert!(SegmentUrl::parse("https://a.googlevideo.com/sq/12").is_err());
        assert!(SegmentUrl::parse("https://a.googlevideo.com/sq/x/seg.ts").is_err());
        assert!(SegmentUrl::parse("https://example.com/sq/12/seg.ts").is_err());
        assert!(SegmentUrl::parse("http://a.googlevideo.com/sq/12/seg.ts").is_err());
    }
}
