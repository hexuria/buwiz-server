//! Jev (TypeSafe System One) as a cheap second opinion on a claim file.
//!
//! Two yes/no questions: is this a Form 2303, and was it edited. TypeSafe
//! takes the photo itself; a gateway's `/v1/decisions` takes text only, so
//! there Jev judges the reader's transcript.

use std::time::Duration;

use typesafe_sdk::{Client, Compression, Image, Question, RetryPolicy, SystemOneOpts, Transport};

use super::config::{ClassifierConfig, ClassifierRoute};
use crate::domain::claim_assessment::SecondOpinion;
use crate::domain::provenance::UploadKind;

const FORM_2303: &str = "Is this a BIR Form 2303 Certificate of Registration issued by the Philippine \
Bureau of Internal Revenue?";
const EDITED: &str = "Does it show signs of digital editing or forgery, such as text in mismatched \
fonts or sizes, misaligned or pasted fields, or a missing BIR header, seal or barcode?";

/// What Jev is shown.
pub enum JevInput<'a> {
    Photo { kind: UploadKind, bytes: &'a [u8] },
    Text(&'a str),
}

/// The input Jev can take on this route, if any: PDFs and text-only routes
/// need the reader's transcript.
pub fn input_for<'a>(
    route: ClassifierRoute,
    kind: UploadKind,
    file: &'a [u8],
    transcript: Option<&'a str>,
) -> Option<JevInput<'a>> {
    let transcript = transcript.map(str::trim).filter(|text| !text.is_empty());
    match (route, kind) {
        (ClassifierRoute::SystemOne, UploadKind::Jpeg | UploadKind::Png) => {
            Some(JevInput::Photo { kind, bytes: file })
        }
        _ => transcript.map(JevInput::Text),
    }
}

pub async fn second_opinion(
    config: &ClassifierConfig,
    input: JevInput<'_>,
) -> Result<SecondOpinion, String> {
    second_opinion_via(config, input, super::transport::SpinTransport { route: config.route }).await
}

async fn second_opinion_via(
    config: &ClassifierConfig,
    input: JevInput<'_>,
    transport: impl Transport,
) -> Result<SecondOpinion, String> {
    let client = Client::builder()
        .api_key(config.endpoint.api_key.clone())
        .base_url(config.endpoint.base_url.clone())
        .model(config.endpoint.model.clone())
        .timeout(Duration::from_secs(30))
        .compression(Compression::None)
        .retry(RetryPolicy { max_retries: 1, ..RetryPolicy::default() })
        .transport(transport)
        .build()
        .map_err(|error| format!("Jev client: {error}"))?;
    let questions = [("form_2303", Question::noul(FORM_2303)), ("edited", Question::noul(EDITED))];
    let response = match input {
        JevInput::Photo { bytes, .. } => {
            let photo = Image::from_bytes(bytes.to_vec());
            let mut extra_headers = http::HeaderMap::new();
            extra_headers.insert("typesafe-danger-zone-images", http::HeaderValue::from_static("true"));
            client
                .system_one_opts(
                    format!("Uploaded file: {photo}"),
                    questions,
                    SystemOneOpts { images: vec![photo], extra_headers, ..SystemOneOpts::default() },
                )
                .await
        }
        JevInput::Text(text) => {
            client.system_one(format!("Text read from an uploaded file:\n{text}"), questions).await
        }
    }
    .map_err(|error| format!("Jev: {error}"))?;
    let noul = |name: &str| {
        response.noul(name).map(|answer| answer.noul).map_err(|error| format!("Jev: {error}"))
    };
    Ok(SecondOpinion {
        source: response.model.clone(),
        form_2303: noul("form_2303")?,
        edited: noul("edited")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn photos_go_to_typesafe_and_everything_else_needs_the_transcript() {
        let photo = input_for(ClassifierRoute::SystemOne, UploadKind::Png, b"png", None);
        assert!(matches!(photo, Some(JevInput::Photo { .. })));
        let pdf = input_for(ClassifierRoute::SystemOne, UploadKind::Pdf, b"%PDF", Some("COR"));
        assert!(matches!(pdf, Some(JevInput::Text("COR"))));
        let gateway = input_for(ClassifierRoute::Decisions, UploadKind::Jpeg, b"jpg", Some(" "));
        assert!(gateway.is_none(), "a gateway cannot take the photo and there is no text");
    }

    /// Answers every attempt with HTTP 500 and keeps what it was sent.
    #[derive(Clone, Default)]
    struct Failing(std::sync::Arc<std::sync::Mutex<Vec<typesafe_sdk::TransportRequest>>>);

    impl Transport for Failing {
        fn send(
            &self,
            request: typesafe_sdk::TransportRequest,
        ) -> typesafe_sdk::transport::BoxFuture<'_, Result<typesafe_sdk::TransportResponse, typesafe_sdk::Error>>
        {
            self.0.lock().unwrap().push(request);
            Box::pin(async {
                Ok(typesafe_sdk::TransportResponse {
                    status: 500,
                    headers: http::HeaderMap::new(),
                    body: bytes::Bytes::from_static(b"{}"),
                })
            })
        }

        fn sleep(&self, _: Duration) -> typesafe_sdk::transport::BoxFuture<'_, ()> {
            Box::pin(async {})
        }
    }

    fn classifier() -> ClassifierConfig {
        ClassifierConfig {
            route: ClassifierRoute::SystemOne,
            endpoint: super::super::config::Endpoint {
                base_url: "https://api.typesafe.test".into(),
                api_key: "ts_test".into(),
                model: "jev-latest".into(),
            },
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_photo_goes_with_the_images_header_and_is_retried_once() {
        let transport = Failing::default();
        let photo = b"\x89PNG\r\n\x1a\nphoto-bytes";
        let input = JevInput::Photo { kind: UploadKind::Png, bytes: photo };
        let error = second_opinion_via(&classifier(), input, transport.clone()).await.unwrap_err();
        assert!(error.starts_with("Jev:"), "{error}");
        let sent = transport.0.lock().unwrap();
        assert_eq!(sent.len(), 2, "one try and one retry");
        let first = &sent[0];
        assert_eq!(first.headers["typesafe-danger-zone-images"], "true");
        let body = first.body.as_deref().unwrap_or_default();
        let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, photo);
        let has = |needle: &[u8]| body.windows(needle.len()).any(|w| w == needle);
        assert!(has(b"photo-bytes") || has(encoded.as_bytes()), "the photo is in the request");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn outside_spin_jev_is_unreachable() {
        let error = second_opinion(&classifier(), JevInput::Text("COR")).await.unwrap_err();
        assert!(error.starts_with("Jev:") && error.contains("Spin runtime"), "{error}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn text_goes_without_the_images_header() {
        let transport = Failing::default();
        let _ = second_opinion_via(&classifier(), JevInput::Text("COR"), transport.clone()).await;
        let sent = transport.0.lock().unwrap();
        assert!(!sent.is_empty());
        assert!(sent[0].headers.get("typesafe-danger-zone-images").is_none());
    }
}
