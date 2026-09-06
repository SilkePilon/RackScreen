//! HTTP/1.1 client over a kube port-forward, so no kubectl binary is needed on the Pi.

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::client::conn::http1::{self, SendRequest};
use hyper::{HeaderMap, Request};
use hyper_util::rt::TokioIo;
use k8s_openapi::api::core::v1::{Pod, Service};
use kube::api::{ListParams, Portforwarder};
use kube::{Api, Client};

pub struct Response {
    pub status: u16,
    pub headers: HeaderMap,
    pub body: String,
}

pub struct Tunnel {
    send: SendRequest<Full<Bytes>>,
    _pf: Portforwarder,
}

impl Tunnel {
    pub async fn open(client: &Client, ns: &str, pod: &str, port: u16) -> Result<Tunnel> {
        let pods: Api<Pod> = Api::namespaced(client.clone(), ns);
        let mut pf = pods
            .portforward(pod, &[port])
            .await
            .with_context(|| format!("port-forward {ns}/{pod}:{port}"))?;
        let stream = pf.take_stream(port).context("port-forward stream")?;
        let (send, conn) = http1::handshake(TokioIo::new(stream))
            .await
            .context("http handshake")?;
        tokio::spawn(async move {
            if let Err(e) = conn.await {
                tracing::debug!("tunnel connection closed: {e}");
            }
        });
        Ok(Tunnel { send, _pf: pf })
    }

    async fn send(&mut self, req: Request<Full<Bytes>>) -> Result<Response> {
        self.send.ready().await.context("tunnel not ready")?;
        let resp = self.send.send_request(req).await.context("request")?;
        let status = resp.status().as_u16();
        let headers = resp.headers().clone();
        let bytes = resp.into_body().collect().await.context("body")?.to_bytes();
        Ok(Response {
            status,
            headers,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        })
    }

    pub async fn get(&mut self, path: &str, headers: &[(&str, &str)]) -> Result<Response> {
        let b = build("GET", path, headers);
        self.send(b.body(Full::new(Bytes::new()))?).await
    }

    pub async fn post_form(
        &mut self,
        path: &str,
        body: &str,
        headers: &[(&str, &str)],
    ) -> Result<Response> {
        let b = build("POST", path, headers)
            .header("content-type", "application/x-www-form-urlencoded");
        self.send(b.body(Full::new(Bytes::from(body.to_owned())))?)
            .await
    }
}

/// hyper's HTTP/1 client serializes the URI verbatim, so it must be origin-form
/// (`/path?query`). Absolute-form request lines make servers like qBittorrent 404.
fn build(method: &str, path: &str, headers: &[(&str, &str)]) -> hyper::http::request::Builder {
    let mut b = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "localhost");
    for (k, v) in headers {
        b = b.header(*k, *v);
    }
    b
}

/// Resolve a service (exact name, or "auto" = first service whose name contains `needle`
/// and exposes `port`) to one Running, Ready pod behind it.
pub async fn find_pod_for_service(
    client: &Client,
    ns: &str,
    service_hint: &str,
    port: u16,
    needle: &str,
) -> Result<(String, String)> {
    let svcs: Api<Service> = Api::namespaced(client.clone(), ns);
    let svc = if service_hint == "auto" {
        let list = svcs
            .list(&ListParams::default())
            .await
            .context("list services")?;
        list.items
            .into_iter()
            .find(|s| {
                let name = s.metadata.name.as_deref().unwrap_or("");
                let has_port = s
                    .spec
                    .as_ref()
                    .and_then(|sp| sp.ports.as_ref())
                    .is_some_and(|ps| ps.iter().any(|p| p.port == port as i32));
                name.contains(needle) && has_port
            })
            .with_context(|| format!("no service containing '{needle}' with port {port} in {ns}"))?
    } else {
        svcs.get(service_hint)
            .await
            .with_context(|| format!("service {ns}/{service_hint}"))?
    };
    let svc_name = svc.metadata.name.clone().unwrap_or_default();
    let selector = svc
        .spec
        .as_ref()
        .and_then(|s| s.selector.as_ref())
        .context("service has no selector")?;
    let label = selector
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(",");
    let pods: Api<Pod> = Api::namespaced(client.clone(), ns);
    let list = pods
        .list(&ListParams::default().labels(&label))
        .await
        .context("list pods")?;
    let pod = list
        .items
        .iter()
        .find(|p| {
            let st = p.status.as_ref();
            st.and_then(|s| s.phase.as_deref()) == Some("Running")
                && st
                    .and_then(|s| s.conditions.as_ref())
                    .is_some_and(|cs| cs.iter().any(|c| c.type_ == "Ready" && c.status == "True"))
        })
        .with_context(|| format!("no ready pod for service {svc_name}"))?;
    Ok((pod.metadata.name.clone().unwrap_or_default(), svc_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_line_is_origin_form() {
        let r = build(
            "GET",
            "/api/v2/torrents/info?filter=downloading",
            &[("referer", "http://localhost:8080")],
        )
        .body(Full::new(Bytes::new()))
        .unwrap();
        assert_eq!(
            r.uri().to_string(),
            "/api/v2/torrents/info?filter=downloading"
        );
        assert_eq!(r.headers()["host"], "localhost");
        assert_eq!(r.headers()["referer"], "http://localhost:8080");
    }
}
