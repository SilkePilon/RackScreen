//! `/api/gifs/*`: GIF search for the GIFs page. The browser never talks to
//! GIPHY or Tenor and never sees the keys: this server calls the fixed API
//! hosts with the keys from `gif_search:` and answers with the few fields
//! the page needs. Answers are cached for a while, so a typing user and a
//! page reload stay inside GIPHY's free-tier rate limit.
//!
//! The link stored for a pick is the one the GIF role downloads best:
//! `https://giphy.com/gifs/<id>` for GIPHY (the classifier turns it into
//! `i.giphy.com/<id>.gif`, no page fetch) and the `media.tenor.com/....gif`
//! file for Tenor (used as is; a `tenor.com/view` page would need a fetch
//! and an HTML scrape on the Pi).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use rackscreen_app::config::{Config, GifSearchCfg};
use rackscreen_core::gif::{classify_link, GifLink};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{ApiError, ApiResult, AppState};

/// Results per page.
pub const PAGE_SIZE: u32 = 25;
/// A provider call may take this long.
pub const TIMEOUT: Duration = Duration::from_secs(8);
/// How long a page of results is reused.
pub const RESULTS_TTL: Duration = Duration::from_secs(10 * 60);
/// How long the popular search terms are reused.
pub const TERMS_TTL: Duration = Duration::from_secs(30 * 60);
/// The most answers kept.
pub const CACHE_MAX: usize = 100;
/// A provider answer is read up to this much.
const MAX_ANSWER_BYTES: usize = 2 * 1024 * 1024;
/// The longest query accepted.
pub const MAX_QUERY_CHARS: usize = 100;
/// GIPHY's own caps: a query of 50 characters, an offset of 4999 (499 for
/// trending).
const GIPHY_MAX_QUERY: usize = 50;
const GIPHY_MAX_OFFSET: u32 = 4999;
const GIPHY_MAX_TRENDING_OFFSET: u32 = 499;
/// Popular terms shown as quick picks.
const MAX_TERMS: usize = 10;
/// Tenor wants every integration to name itself.
pub const TENOR_CLIENT_KEY: &str = "rackscreen";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    Giphy,
    Tenor,
}

impl Provider {
    pub fn parse(s: &str) -> Result<Provider, ApiError> {
        match s {
            "giphy" => Ok(Provider::Giphy),
            "tenor" => Ok(Provider::Tenor),
            _ => Err(ApiError::bad_request("provider must be giphy or tenor")),
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Provider::Giphy => "giphy",
            Provider::Tenor => "tenor",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Provider::Giphy => "GIPHY",
            Provider::Tenor => "Tenor",
        }
    }
    fn key(self, cfg: &GifSearchCfg) -> &str {
        match self {
            Provider::Giphy => cfg.giphy_key.trim(),
            Provider::Tenor => cfg.tenor_key.trim(),
        }
    }
}

/// One search result as the page gets it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Hit {
    pub id: String,
    pub title: String,
    /// The link to store under `gifs:`; always passes `classify_link`.
    pub url: String,
    /// A small animated preview for the grid.
    pub preview: String,
    /// The preview's size, for a box that does not jump while it loads.
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Page {
    pub results: Vec<Hit>,
    /// Pass back as `offset` for the next page; `None` at the end. A number
    /// for GIPHY, Tenor's own opaque position for Tenor.
    pub next: Option<String>,
}

/// What a request asks a provider for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ask {
    Search { q: String, offset: Option<String> },
    Trending { offset: Option<String> },
    Terms,
}

impl Ask {
    fn cache_key(&self, p: Provider, rating: &str) -> String {
        let off = |o: &Option<String>| o.clone().unwrap_or_default();
        match self {
            Ask::Search { q, offset } => format!("{}|{rating}|s|{}|{q}", p.id(), off(offset)),
            Ask::Trending { offset } => format!("{}|{rating}|t|{}", p.id(), off(offset)),
            Ask::Terms => format!("{}|terms", p.id()),
        }
    }
    fn ttl(&self) -> Duration {
        match self {
            Ask::Terms => TERMS_TTL,
            _ => RESULTS_TTL,
        }
    }
}

/// Answers by request, each until its expiry; at most `CACHE_MAX`.
#[derive(Default)]
pub struct SearchCache {
    map: Mutex<HashMap<String, (Instant, Value)>>,
}

impl SearchCache {
    pub fn get(&self, key: &str, now: Instant) -> Option<Value> {
        let map = self.map.lock().unwrap();
        map.get(key)
            .filter(|(exp, _)| *exp > now)
            .map(|(_, v)| v.clone())
    }

    pub fn put(&self, key: String, value: Value, ttl: Duration, now: Instant) {
        let mut map = self.map.lock().unwrap();
        map.retain(|_, (exp, _)| *exp > now);
        while map.len() >= CACHE_MAX {
            // drop the one closest to expiring
            let Some(k) = map
                .iter()
                .min_by_key(|(_, (exp, _))| *exp)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            map.remove(&k);
        }
        map.insert(key, (now + ttl, value));
    }

    pub fn len(&self) -> usize {
        self.map.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ---------------------------------------------------------------------------
// Requests: the fixed hosts and paths, the key only in the query string.

/// GIPHY's `rating` and Tenor's `contentfilter` for a `gif_search.rating`.
fn tenor_filter(rating: &str) -> &'static str {
    match rating {
        "pg" => "medium",
        "pg-13" => "low",
        "r" => "off",
        _ => "high",
    }
}

/// The provider URL for `ask`. Built from fixed parts: only the query
/// string carries the user's input (encoded) and the key.
pub fn request_url(p: Provider, ask: &Ask, key: &str, rating: &str) -> reqwest::Url {
    let (base, q, offset) = match (p, ask) {
        (Provider::Giphy, Ask::Search { q, offset }) => {
            ("https://api.giphy.com/v1/gifs/search", Some(q), offset)
        }
        (Provider::Giphy, Ask::Trending { offset }) => {
            ("https://api.giphy.com/v1/gifs/trending", None, offset)
        }
        (Provider::Giphy, Ask::Terms) => {
            ("https://api.giphy.com/v1/trending/searches", None, &None)
        }
        (Provider::Tenor, Ask::Search { q, offset }) => {
            ("https://tenor.googleapis.com/v2/search", Some(q), offset)
        }
        (Provider::Tenor, Ask::Trending { offset }) => {
            ("https://tenor.googleapis.com/v2/featured", None, offset)
        }
        (Provider::Tenor, Ask::Terms) => (
            "https://tenor.googleapis.com/v2/trending_terms",
            None,
            &None,
        ),
    };
    let mut url = reqwest::Url::parse(base).expect("fixed url");
    {
        let mut qp = url.query_pairs_mut();
        match p {
            Provider::Giphy => {
                qp.append_pair("api_key", key);
                if !matches!(ask, Ask::Terms) {
                    qp.append_pair("limit", &PAGE_SIZE.to_string());
                    qp.append_pair("rating", rating);
                    qp.append_pair("bundle", "messaging_non_clips");
                }
                if let Some(q) = q {
                    let q: String = q.chars().take(GIPHY_MAX_QUERY).collect();
                    qp.append_pair("q", &q);
                }
                if let Some(o) = offset {
                    qp.append_pair("offset", o);
                }
            }
            Provider::Tenor => {
                qp.append_pair("key", key);
                qp.append_pair("client_key", TENOR_CLIENT_KEY);
                if matches!(ask, Ask::Terms) {
                    qp.append_pair("limit", &MAX_TERMS.to_string());
                } else {
                    qp.append_pair("limit", &PAGE_SIZE.to_string());
                    qp.append_pair("media_filter", "gif,tinygif");
                    qp.append_pair("contentfilter", tenor_filter(rating));
                }
                if let Some(q) = q {
                    qp.append_pair("q", q);
                }
                if let Some(o) = offset {
                    qp.append_pair("pos", o);
                }
            }
        }
    }
    url
}

fn client() -> reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT
        .get_or_init(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
            reqwest::Client::builder()
                .timeout(TIMEOUT)
                .user_agent(concat!("rackscreen/", env!("CARGO_PKG_VERSION")))
                // the APIs answer directly; a redirect is not followed
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("reqwest client")
        })
        .clone()
}

fn bad_gateway(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_GATEWAY, msg.into())
}

/// A short message for a provider's error answer. Never the URL (it holds
/// the key) nor the body verbatim.
pub fn provider_error(p: Provider, status: u16, body: &str) -> String {
    let name = p.title();
    let key_refused = match p {
        Provider::Giphy => matches!(status, 401 | 403),
        Provider::Tenor => {
            status == 401
                || (status == 400 && body.contains("API key not valid"))
                || (status == 403 && body.contains("API_KEY_INVALID"))
        }
    };
    if key_refused {
        return format!("{name} rejected the API key");
    }
    match status {
        403 if p == Provider::Tenor => {
            "Tenor refused the key; is the Tenor API enabled for it in Google Cloud?".into()
        }
        429 => format!("{name} rate limit reached; try again in a minute"),
        500..=599 => format!("{name} is having trouble ({status}); try again later"),
        _ => format!("{name} answered {status}"),
    }
}

async fn fetch(p: Provider, url: reqwest::Url) -> Result<Value, ApiError> {
    let name = p.title();
    let resp = client().get(url).send().await.map_err(|e| {
        // the error's text holds the URL, and with it the key
        if e.is_timeout() {
            bad_gateway(format!("{name} did not answer in {} s", TIMEOUT.as_secs()))
        } else {
            bad_gateway(format!("{name} is unreachable"))
        }
    })?;
    let status = resp.status();
    let body = rackscreen_sources::http::read_capped(resp, MAX_ANSWER_BYTES, name)
        .await
        .map_err(|_| bad_gateway(format!("{name} sent an unreadable answer")))?;
    if !status.is_success() {
        let msg = provider_error(p, status.as_u16(), &String::from_utf8_lossy(&body));
        tracing::warn!("gif search: {msg}");
        return Err(bad_gateway(msg));
    }
    serde_json::from_slice(&body)
        .map_err(|_| bad_gateway(format!("{name} sent an unreadable answer")))
}

// ---------------------------------------------------------------------------
// Answers.

fn str_at<'a>(v: &'a Value, path: &[&str]) -> Option<&'a str> {
    path.iter().try_fold(v, |cur, k| cur.get(k))?.as_str()
}

/// A dimension that GIPHY sends as a string and Tenor as a number.
fn dim(v: Option<&Value>) -> Option<u32> {
    match v? {
        Value::String(s) => s.parse().ok(),
        Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        _ => None,
    }
}

fn https(u: &str) -> bool {
    u.starts_with("https://")
}

/// GIPHY's `/v1/gifs/search` or `/trending` answer. `max_offset` is GIPHY's
/// cap for the endpoint; no `next` past it.
pub fn parse_giphy(v: &Value, max_offset: u32) -> Result<Page, String> {
    let data = v
        .get("data")
        .and_then(Value::as_array)
        .ok_or("no data in the GIPHY answer")?;
    let mut results = Vec::with_capacity(data.len());
    for g in data {
        let Some(id) = g.get("id").and_then(Value::as_str) else {
            continue;
        };
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
            continue;
        }
        let url = format!("https://giphy.com/gifs/{id}");
        if classify_link(&url).is_err() {
            continue;
        }
        let images = g.get("images");
        let Some(img) = [
            "fixed_width_small",
            "fixed_width",
            "fixed_height_small",
            "original",
        ]
        .iter()
        .filter_map(|k| images.and_then(|i| i.get(*k)))
        .find(|i| i.get("url").and_then(Value::as_str).is_some_and(https)) else {
            continue;
        };
        results.push(Hit {
            id: id.to_string(),
            title: g
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string(),
            url,
            preview: str_at(img, &["url"]).unwrap_or_default().to_string(),
            width: dim(img.get("width")).unwrap_or(100),
            height: dim(img.get("height")).unwrap_or(100),
        });
    }
    let pg = v.get("pagination");
    let num = |k: &str| pg.and_then(|p| p.get(k)).and_then(Value::as_u64);
    let offset = num("offset").unwrap_or(0);
    let count = num("count").unwrap_or(data.len() as u64);
    let next = offset + count;
    let more =
        count > 0 && num("total_count").is_none_or(|t| next < t) && next <= max_offset as u64;
    Ok(Page {
        results,
        next: more.then(|| next.to_string()),
    })
}

/// Tenor's `/v2/search` or `/featured` answer.
pub fn parse_tenor(v: &Value) -> Result<Page, String> {
    let data = v
        .get("results")
        .and_then(Value::as_array)
        .ok_or("no results in the Tenor answer")?;
    let mut results = Vec::with_capacity(data.len());
    for g in data {
        let Some(id) = g.get("id").and_then(Value::as_str) else {
            continue;
        };
        let media = g.get("media_formats");
        let gif = media.and_then(|m| m.get("gif"));
        let Some(url) = gif.and_then(|g| g.get("url")).and_then(Value::as_str) else {
            continue;
        };
        if !matches!(classify_link(url), Ok(GifLink::Direct(_))) {
            continue;
        }
        let Some(small) = ["tinygif", "nanogif", "gif"]
            .iter()
            .filter_map(|k| media.and_then(|m| m.get(*k)))
            .find(|i| i.get("url").and_then(Value::as_str).is_some_and(https))
        else {
            continue;
        };
        let dims = small.get("dims").and_then(Value::as_array);
        let title = [g.get("title"), g.get("content_description")]
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::trim)
            .find(|s| !s.is_empty())
            .unwrap_or("");
        results.push(Hit {
            id: id.to_string(),
            title: title.to_string(),
            url: url.to_string(),
            preview: str_at(small, &["url"]).unwrap_or_default().to_string(),
            width: dim(dims.and_then(|d| d.first())).unwrap_or(100),
            height: dim(dims.and_then(|d| d.get(1))).unwrap_or(100),
        });
    }
    let next = v
        .get("next")
        .and_then(|n| match n {
            Value::String(s) => Some(s.trim().to_string()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        })
        .filter(|s| !s.is_empty() && s != "0" && !data.is_empty());
    Ok(Page { results, next })
}

/// Popular search terms: GIPHY's `data` or Tenor's `results`, strings.
pub fn parse_terms(p: Provider, v: &Value) -> Result<Vec<String>, String> {
    let list = v
        .get(match p {
            Provider::Giphy => "data",
            Provider::Tenor => "results",
        })
        .and_then(Value::as_array)
        .ok_or_else(|| format!("no terms in the {} answer", p.title()))?;
    let mut out: Vec<String> = Vec::new();
    for t in list.iter().filter_map(Value::as_str) {
        let t = t.trim();
        if t.is_empty() || t.chars().count() > GIPHY_MAX_QUERY || out.iter().any(|o| o == t) {
            continue;
        }
        out.push(t.to_string());
        if out.len() == MAX_TERMS {
            break;
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Handlers.

#[derive(Deserialize)]
pub struct SearchQuery {
    pub provider: String,
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub offset: Option<String>,
}

#[derive(Deserialize)]
pub struct ProviderQuery {
    pub provider: String,
}

fn search_cfg(state: &AppState) -> Result<GifSearchCfg, ApiError> {
    Config::load_or_default(&state.hub.config_path)
        .map(|c| c.gif_search)
        .map_err(|e| ApiError::internal(format!("{e:#}")))
}

/// The offset a request may send: for GIPHY a number within its cap, for
/// Tenor its own position token (short, URL-safe characters).
pub fn check_offset(
    p: Provider,
    offset: Option<&str>,
    max: u32,
) -> Result<Option<String>, ApiError> {
    let Some(o) = offset.map(str::trim).filter(|o| !o.is_empty()) else {
        return Ok(None);
    };
    let ok = match p {
        Provider::Giphy => o.parse::<u32>().is_ok_and(|n| n <= max),
        Provider::Tenor => {
            o.len() <= 200
                && o.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "._-=+/:".contains(c))
        }
    };
    if !ok {
        return Err(ApiError::bad_request("bad offset"));
    }
    Ok(Some(o.to_string()))
}

async fn answer(state: &AppState, p: Provider, ask: Ask) -> Result<Value, ApiError> {
    let cfg = search_cfg(state)?;
    let key = p.key(&cfg);
    if key.is_empty() {
        return Err(ApiError::bad_request(format!(
            "No {} API key: add one under GIF search and save",
            p.title()
        )));
    }
    let cache_key = ask.cache_key(p, &cfg.rating);
    if let Some(v) = state.gif_cache.get(&cache_key, Instant::now()) {
        return Ok(v);
    }
    let url = request_url(p, &ask, key, &cfg.rating);
    let raw = fetch(p, url).await?;
    let parsed = match (&ask, p) {
        (Ask::Terms, _) => parse_terms(p, &raw).map(|t| json!({ "terms": t })),
        (Ask::Trending { .. }, Provider::Giphy) => {
            parse_giphy(&raw, GIPHY_MAX_TRENDING_OFFSET).map(|pg| json!(pg))
        }
        (_, Provider::Giphy) => parse_giphy(&raw, GIPHY_MAX_OFFSET).map(|pg| json!(pg)),
        (_, Provider::Tenor) => parse_tenor(&raw).map(|pg| json!(pg)),
    }
    .map_err(bad_gateway)?;
    state
        .gif_cache
        .put(cache_key, parsed.clone(), ask.ttl(), Instant::now());
    Ok(parsed)
}

/// `{ "giphy": bool, "tenor": bool }`: which providers have a key.
pub async fn providers(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    let cfg = search_cfg(&state)?;
    Ok(Json(json!({
        "giphy": !cfg.giphy_key.trim().is_empty(),
        "tenor": !cfg.tenor_key.trim().is_empty(),
    })))
}

pub async fn search(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SearchQuery>,
) -> ApiResult<Value> {
    let p = Provider::parse(&q.provider)?;
    let text = q.q.as_deref().unwrap_or("").trim().to_string();
    let n = text.chars().count();
    if n == 0 || n > MAX_QUERY_CHARS {
        return Err(ApiError::bad_request(format!(
            "q must be 1 to {MAX_QUERY_CHARS} characters"
        )));
    }
    let offset = check_offset(p, q.offset.as_deref(), GIPHY_MAX_OFFSET)?;
    Ok(Json(
        answer(&state, p, Ask::Search { q: text, offset }).await?,
    ))
}

pub async fn trending(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SearchQuery>,
) -> ApiResult<Value> {
    let p = Provider::parse(&q.provider)?;
    let offset = check_offset(p, q.offset.as_deref(), GIPHY_MAX_TRENDING_OFFSET)?;
    Ok(Json(answer(&state, p, Ask::Trending { offset }).await?))
}

/// `{ "terms": [...] }`: what people search for right now.
pub async fn trending_terms(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ProviderQuery>,
) -> ApiResult<Value> {
    let p = Provider::parse(&q.provider)?;
    Ok(Json(answer(&state, p, Ask::Terms).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Value {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
    }

    #[test]
    fn giphy_search_answer_becomes_page_links_and_previews() {
        let page = parse_giphy(&fixture("giphy-search.json"), GIPHY_MAX_OFFSET).unwrap();
        assert_eq!(page.results.len(), 2, "the entry with a bad id is skipped");
        let h = &page.results[0];
        assert_eq!(h.id, "3o7TKSjRrfIPjeiVyM");
        assert_eq!(h.url, "https://giphy.com/gifs/3o7TKSjRrfIPjeiVyM");
        assert_eq!(
            classify_link(&h.url),
            Ok(GifLink::Direct(
                "https://i.giphy.com/3o7TKSjRrfIPjeiVyM.gif".into()
            ))
        );
        assert_eq!(
            h.preview,
            "https://media2.giphy.com/media/v1.Y2lkPTc5MGI3NjEx/3o7TKSjRrfIPjeiVyM/100w.gif?cid=790b7611&rid=100w.gif&ct=g"
        );
        assert_eq!((h.width, h.height), (100, 56), "string dims parse");
        assert_eq!(h.title, "Happy Cat GIF by Cheezburger");
        assert_eq!(page.results[1].title, "", "a missing title is empty");
        assert_eq!(page.next.as_deref(), Some("28"), "offset 3 + count 25");
    }

    #[test]
    fn giphy_stops_at_the_last_page_and_at_its_offset_cap() {
        let mut v = fixture("giphy-search.json");
        v["pagination"] = json!({"total_count": 28, "count": 25, "offset": 3});
        assert_eq!(parse_giphy(&v, GIPHY_MAX_OFFSET).unwrap().next, None);
        v["pagination"] = json!({"total_count": 9000, "count": 25, "offset": 475});
        assert_eq!(
            parse_giphy(&v, GIPHY_MAX_TRENDING_OFFSET).unwrap().next,
            None,
            "500 is past trending's 499"
        );
        assert!(parse_giphy(&json!({"meta": {}}), GIPHY_MAX_OFFSET).is_err());
    }

    #[test]
    fn tenor_search_answer_stores_the_media_gif() {
        let page = parse_tenor(&fixture("tenor-search.json")).unwrap();
        assert_eq!(page.results.len(), 2, "the entry without a gif is skipped");
        let h = &page.results[0];
        assert_eq!(h.id, "16596288935386558830");
        assert_eq!(
            h.url,
            "https://media.tenor.com/5lLcKZgmIhgAAAAC/excited-happy.gif"
        );
        assert!(matches!(classify_link(&h.url), Ok(GifLink::Direct(_))));
        assert_eq!(
            h.preview,
            "https://media.tenor.com/5lLcKZgmIhgAAAAM/excited-happy.gif"
        );
        assert_eq!((h.width, h.height), (220, 124));
        assert_eq!(h.title, "Excited Happy GIF", "empty title: the description");
        assert_eq!(page.results[1].title, "Party");
        assert_eq!(page.next.as_deref(), Some("CAgQ7gEaHgoKAD-_xTqJ4oQdEhD"));
        let mut v = fixture("tenor-search.json");
        v["next"] = json!("");
        assert_eq!(parse_tenor(&v).unwrap().next, None);
    }

    #[test]
    fn trending_terms_from_both_providers() {
        let g =
            json!({"data": ["cat", " dog ", "", "cat", "x".repeat(60)], "meta": {"status": 200}});
        assert_eq!(parse_terms(Provider::Giphy, &g).unwrap(), ["cat", "dog"]);
        let t = json!({"results": ["happy birthday", "good morning"], "next": "0"});
        assert_eq!(
            parse_terms(Provider::Tenor, &t).unwrap(),
            ["happy birthday", "good morning"]
        );
        let many = json!({"data": (0..30).map(|i| format!("t{i}")).collect::<Vec<_>>()});
        assert_eq!(
            parse_terms(Provider::Giphy, &many).unwrap().len(),
            MAX_TERMS
        );
    }

    #[test]
    fn requests_go_to_the_fixed_hosts_with_the_key_in_the_query() {
        let s = Ask::Search {
            q: "dance & party?/x".into(),
            offset: Some("25".into()),
        };
        let u = request_url(Provider::Giphy, &s, "GKEY", "pg-13");
        assert_eq!(u.host_str(), Some("api.giphy.com"));
        assert_eq!(u.path(), "/v1/gifs/search");
        let pairs: HashMap<_, _> = u.query_pairs().into_owned().collect();
        assert_eq!(pairs["api_key"], "GKEY");
        assert_eq!(pairs["q"], "dance & party?/x", "encoded, not split");
        assert_eq!(pairs["offset"], "25");
        assert_eq!(pairs["rating"], "pg-13");
        assert_eq!(pairs["limit"], "25");

        let t = request_url(
            Provider::Tenor,
            &Ask::Trending { offset: None },
            "TKEY",
            "g",
        );
        assert_eq!(t.host_str(), Some("tenor.googleapis.com"));
        assert_eq!(t.path(), "/v2/featured");
        let pairs: HashMap<_, _> = t.query_pairs().into_owned().collect();
        assert_eq!(pairs["key"], "TKEY");
        assert_eq!(pairs["client_key"], "rackscreen");
        assert_eq!(pairs["media_filter"], "gif,tinygif");
        assert_eq!(pairs["contentfilter"], "high");
        assert!(!pairs.contains_key("pos"));

        let terms = request_url(Provider::Tenor, &Ask::Terms, "TKEY", "g");
        assert_eq!(terms.path(), "/v2/trending_terms");
        let terms = request_url(Provider::Giphy, &Ask::Terms, "GKEY", "g");
        assert_eq!(terms.path(), "/v1/trending/searches");
        let long = Ask::Search {
            q: "a".repeat(100),
            offset: None,
        };
        let u = request_url(Provider::Giphy, &long, "K", "g");
        let pairs: HashMap<_, _> = u.query_pairs().into_owned().collect();
        assert_eq!(pairs["q"].len(), GIPHY_MAX_QUERY, "GIPHY's own cap");
    }

    #[test]
    fn provider_errors_are_short_and_never_hold_the_key() {
        assert_eq!(
            provider_error(
                Provider::Giphy,
                401,
                r#"{"message":"Invalid authentication credentials"}"#
            ),
            "GIPHY rejected the API key"
        );
        assert_eq!(
            provider_error(
                Provider::Tenor,
                400,
                r#"{"error":{"code":400,"message":"API key not valid. Please pass a valid API key.","status":"INVALID_ARGUMENT"}}"#
            ),
            "Tenor rejected the API key"
        );
        assert!(provider_error(Provider::Tenor, 403, "{}").contains("Tenor API enabled"));
        assert!(provider_error(Provider::Giphy, 429, "").contains("rate limit"));
        assert_eq!(
            provider_error(Provider::Tenor, 404, "KEY"),
            "Tenor answered 404"
        );
    }

    #[test]
    fn offsets_are_numbers_for_giphy_and_tokens_for_tenor() {
        assert_eq!(check_offset(Provider::Giphy, None, 4999).unwrap(), None);
        assert_eq!(
            check_offset(Provider::Giphy, Some("50"), 4999)
                .unwrap()
                .as_deref(),
            Some("50")
        );
        assert!(check_offset(Provider::Giphy, Some("5000"), 4999).is_err());
        assert!(check_offset(Provider::Giphy, Some("abc"), 4999).is_err());
        assert!(check_offset(Provider::Tenor, Some("CAgQ7gEa=="), 0).is_ok());
        assert!(check_offset(Provider::Tenor, Some("a&key=x"), 0).is_err());
    }

    #[test]
    fn the_cache_expires_and_stays_small() {
        let c = SearchCache::default();
        let t0 = Instant::now();
        c.put("a".into(), json!(1), Duration::from_secs(10), t0);
        assert_eq!(c.get("a", t0), Some(json!(1)));
        assert_eq!(c.get("a", t0 + Duration::from_secs(11)), None);
        for i in 0..CACHE_MAX + 20 {
            c.put(
                format!("k{i}"),
                json!(i),
                Duration::from_secs(60 + i as u64),
                t0,
            );
        }
        assert_eq!(c.len(), CACHE_MAX);
        assert_eq!(c.get("k0", t0), None, "the soonest to expire went first");
        assert!(c.get(&format!("k{}", CACHE_MAX + 19), t0).is_some());
    }

    #[test]
    fn cache_keys_tell_providers_ratings_and_pages_apart() {
        let s = |o: Option<&str>| Ask::Search {
            q: "cat".into(),
            offset: o.map(Into::into),
        };
        let keys = [
            s(None).cache_key(Provider::Giphy, "g"),
            s(None).cache_key(Provider::Tenor, "g"),
            s(None).cache_key(Provider::Giphy, "r"),
            s(Some("25")).cache_key(Provider::Giphy, "g"),
            Ask::Trending { offset: None }.cache_key(Provider::Giphy, "g"),
            Ask::Terms.cache_key(Provider::Giphy, "g"),
        ];
        let unique: std::collections::HashSet<_> = keys.iter().collect();
        assert_eq!(unique.len(), keys.len());
    }
}
