//! The embedded frontend: `web/dist` (or a placeholder) with an SPA fallback
//! to `index.html`, precompressed siblings when the browser takes them, and
//! long caching for the hashed `/assets/*`.

use axum::body::Body;
use axum::http::header::{ACCEPT_ENCODING, CACHE_CONTROL, CONTENT_ENCODING, CONTENT_TYPE, VARY};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use include_dir::{include_dir, Dir, File};

pub static UI: Dir<'static> = include_dir!("$OUT_DIR/ui");

/// Whether the UI was built in (else the placeholder page).
pub fn ui_built() -> bool {
    cfg!(ui_built)
}

/// The file to serve for `path` and whether it is the SPA fallback.
fn lookup(path: &str) -> Option<(&'static File<'static>, &'static str)> {
    let path = path.trim_start_matches('/');
    if path.is_empty() {
        return UI.get_file("index.html").map(|f| (f, "index.html"));
    }
    if let Some(f) = UI.get_file(path) {
        let name: &'static str = f.path().to_str().unwrap_or("");
        return Some((f, name));
    }
    if UI.get_dir(path).is_some() {
        let idx = format!("{}/index.html", path.trim_end_matches('/'));
        if let Some(f) = UI.get_file(&idx) {
            return Some((f, f.path().to_str().unwrap_or("")));
        }
    }
    // SPA routes (no extension) fall back to the app shell
    if !path.contains('.') || path.ends_with(".html") {
        return UI.get_file("index.html").map(|f| (f, "index.html"));
    }
    None
}

/// The encoding the client accepts for which a precompressed sibling exists.
fn precompressed(
    name: &str,
    headers: &HeaderMap,
) -> Option<(&'static File<'static>, &'static str)> {
    let accept = headers
        .get(ACCEPT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let accepts = |enc: &str| {
        accept
            .split(',')
            .map(|s| s.trim().split(';').next().unwrap_or("").trim())
            .any(|s| s == enc)
    };
    if accepts("br") {
        if let Some(f) = UI.get_file(format!("{name}.br")) {
            return Some((f, "br"));
        }
    }
    if accepts("gzip") {
        if let Some(f) = UI.get_file(format!("{name}.gz")) {
            return Some((f, "gzip"));
        }
    }
    None
}

pub fn response_for(path: &str, headers: &HeaderMap) -> Response {
    let Some((file, name)) = lookup(path) else {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    };
    let mime = mime_guess::from_path(name).first_or_octet_stream();
    let mut ctype = mime.to_string();
    if mime.type_() == mime_guess::mime::TEXT
        || mime.subtype() == mime_guess::mime::JAVASCRIPT
        || mime.subtype() == "json"
    {
        ctype.push_str("; charset=utf-8");
    }
    let (body, encoding) = match precompressed(name, headers) {
        Some((f, enc)) => (f.contents(), Some(enc)),
        None => (file.contents(), None),
    };
    let cache = if name.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    let mut resp = Response::new(Body::from(body));
    let h = resp.headers_mut();
    h.insert(CONTENT_TYPE, HeaderValue::from_str(&ctype).expect("mime"));
    h.insert(CACHE_CONTROL, HeaderValue::from_static(cache));
    h.insert(VARY, HeaderValue::from_static("Accept-Encoding"));
    if let Some(enc) = encoding {
        h.insert(CONTENT_ENCODING, HeaderValue::from_static(enc));
    }
    resp
}

pub async fn serve(uri: Uri, headers: HeaderMap) -> Response {
    response_for(uri.path(), &headers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_and_spa_fallback_are_served_as_html() {
        let h = HeaderMap::new();
        for p in ["/", "/index.html", "/screens", "/system/yaml"] {
            let r = response_for(p, &h);
            assert_eq!(r.status(), StatusCode::OK, "{p}");
            assert!(r.headers()[CONTENT_TYPE]
                .to_str()
                .unwrap()
                .starts_with("text/html"));
            assert_eq!(r.headers()[CACHE_CONTROL], "no-cache");
        }
        let r = response_for("/assets/nope-abc123.js", &h);
        assert_eq!(
            r.status(),
            StatusCode::NOT_FOUND,
            "a missing asset is not the shell"
        );
        let _ = ui_built();
    }
}
