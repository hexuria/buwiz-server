//! Jev (TypeSafe System One) as a cheap second opinion on a claim file.
//!
//! Two yes/no questions: is this a Form 2303, and was it edited. TypeSafe
//! takes the photo itself; a gateway's `/v1/decisions` takes text only, so
//! there Jev judges the reader's transcript.

use std::time::Duration;

use typesafe_sdk::{Client, Compression, Image, Question, RetryPolicy, SystemOneOpts};

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
    let client = Client::builder()
        .api_key(config.endpoint.api_key.clone())
        .base_url(config.endpoint.base_url.clone())
        .model(config.endpoint.model.clone())
        .timeout(Duration::from_secs(30))
        .compression(Compression::None)
        .retry(RetryPolicy { max_retries: 1, ..RetryPolicy::default() })
        .transport(super::transport::SpinTransport { route: config.route })
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
}
