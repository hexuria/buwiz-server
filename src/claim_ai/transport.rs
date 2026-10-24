//! Outbound HTTPS through Spin, for the reader and for the TypeSafe SDK.

use std::time::Duration;

use bytes::Bytes;
use typesafe_sdk::transport::BoxFuture;
use typesafe_sdk::{Error, Transport, TransportRequest, TransportResponse};

use super::config::ClassifierRoute;

pub struct SpinTransport {
    pub route: ClassifierRoute,
}

impl Transport for SpinTransport {
    fn send(&self, mut request: TransportRequest) -> BoxFuture<'_, Result<TransportResponse, Error>> {
        request.url = routed_url(self.route, request.url);
        Box::pin(async move {
            let timeout = request.timeout;
            let (status, headers, body) = send(
                request.method,
                &request.url,
                request.headers,
                request.body.unwrap_or_default(),
                timeout,
            )
            .await
            .map_err(|failure| match failure {
                Failure::Timeout => Error::Timeout { timeout },
                Failure::Connection(message) => Error::Connection { message },
            })?;
            Ok(TransportResponse { status, headers, body })
        })
    }

    fn sleep(&self, duration: Duration) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            #[cfg(all(feature = "postgres", runtime_spin))]
            wasip3::clocks::monotonic_clock::wait_for(duration.as_nanos() as u64).await;
            #[cfg(not(all(feature = "postgres", runtime_spin)))]
            let _ = duration;
        })
    }
}

/// A gateway serves the System One body at `/v1/decisions`.
fn routed_url(route: ClassifierRoute, url: String) -> String {
    match url.strip_suffix("/v1/systemone") {
        Some(base) if route == ClassifierRoute::Decisions => format!("{base}/v1/decisions"),
        _ => url,
    }
}

pub enum Failure {
    Timeout,
    Connection(String),
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout => f.write_str("timed out"),
            Self::Connection(message) => f.write_str(message),
        }
    }
}

/// One request; every status is returned, not turned into an error.
pub async fn send(
    method: http::Method,
    url: &str,
    headers: http::HeaderMap,
    body: Bytes,
    timeout: Duration,
) -> Result<(u16, http::HeaderMap, Bytes), Failure> {
    #[cfg(all(feature = "postgres", runtime_spin))]
    {
        use futures::future::{Either, select};
        use http_body_util::BodyExt;
        use spin_sdk::http::{FullBody, send};

        let mut builder = http::Request::builder().method(method).uri(url);
        if let Some(target) = builder.headers_mut() {
            *target = headers;
        }
        let request = builder
            .body(FullBody::new(body))
            .map_err(|error| Failure::Connection(format!("request build failed: {error}")))?;
        let exchange = async move {
            let response = send(request)
                .await
                .map_err(|error| Failure::Connection(format!("request failed: {error}")))?;
            let status = response.status().as_u16();
            let headers = response.headers().clone();
            let body = response
                .into_body()
                .collect()
                .await
                .map_err(|error| Failure::Connection(format!("response body failed: {error:?}")))?
                .to_bytes();
            Ok((status, headers, body))
        };
        let timer = wasip3::clocks::monotonic_clock::wait_for(timeout.as_nanos() as u64);
        match select(Box::pin(exchange), Box::pin(timer)).await {
            Either::Left((result, _)) => result,
            Either::Right(_) => Err(Failure::Timeout),
        }
    }

    #[cfg(not(all(feature = "postgres", runtime_spin)))]
    {
        let _ = (method, url, headers, body, timeout);
        Err(Failure::Connection("AI requests need the Spin runtime".to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_decisions_route_moves_system_one_calls() {
        let url = "https://gw.test/v1/systemone".to_owned();
        assert_eq!(routed_url(ClassifierRoute::Decisions, url.clone()), "https://gw.test/v1/decisions");
        assert_eq!(routed_url(ClassifierRoute::SystemOne, url.clone()), url);
        let other = "https://gw.test/v1/models".to_owned();
        assert_eq!(routed_url(ClassifierRoute::Decisions, other.clone()), other);
    }

    #[test]
    fn failures_read_plainly() {
        assert_eq!(Failure::Timeout.to_string(), "timed out");
        assert_eq!(Failure::Connection("refused".into()).to_string(), "refused");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn outside_spin_nothing_is_sent() {
        let sent = send(http::Method::GET, "https://x.test", http::HeaderMap::new(), Bytes::new(), Duration::from_secs(1)).await;
        assert_eq!(sent.err().map(|f| f.to_string()).as_deref(), Some("AI requests need the Spin runtime"));
    }
}
