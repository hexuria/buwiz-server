//! AI help for the Form 2303 review queue.
//!
//! A reader model ([`reader`]: Anthropic, OpenAI or Gemini shape) reads the
//! file into fields; Jev ([`jev`]) gives yes/no probabilities on the same
//! file. `domain::claim_assessment` turns both into a recommendation with
//! fixed rules. Staff decide every claim; nothing here approves one.
//!
//! The file goes to the configured providers only. Use a gateway with zero
//! data retention for real taxpayer documents.

pub mod config;
pub mod jev;
pub mod reader;
pub(crate) mod transport;

use std::collections::HashMap;
use std::time::Duration;

use bytes::Bytes;

use crate::domain::claim_assessment::{
    DocumentReading, SecondOpinion, fold_second_opinion, reading_from_opinion,
};
use crate::domain::provenance::UploadKind;
use config::AssessmentConfig;

const VARIABLES: [&str; 11] = [
    "AI_GATEWAY_BASE_URL",
    "AI_GATEWAY_API_KEY",
    "AI_READER_SHAPE",
    "AI_READER_BASE_URL",
    "AI_READER_API_KEY",
    "AI_READER_MODEL",
    "AI_CLASSIFIER",
    "AI_CLASSIFIER_ROUTE",
    "AI_CLASSIFIER_BASE_URL",
    "AI_CLASSIFIER_API_KEY",
    "AI_CLASSIFIER_MODEL",
];

const READER_TIMEOUT: Duration = Duration::from_secs(60);

pub async fn load_config() -> Result<AssessmentConfig, String> {
    let mut values = HashMap::new();
    for name in VARIABLES {
        if let Some(value) = crate::store::runtime_config_value(name).await {
            values.insert(name, value);
        }
    }
    config::resolve(|name| values.get(name).cloned())
}

/// What the models made of one file.
#[derive(Debug, Default)]
pub struct ModelRun {
    /// `None` when neither model produced anything usable.
    pub reading: Option<DocumentReading>,
    pub reader_model: Option<String>,
    pub classifier_model: Option<String>,
    pub problems: Vec<String>,
}

pub async fn read_and_classify(config: &AssessmentConfig, kind: UploadKind, file: &[u8]) -> ModelRun {
    let mut run = ModelRun::default();
    let mut read = None;
    if let Some(reader_config) = &config.reader {
        run.reader_model = Some(reader_config.endpoint.model.clone());
        match call_reader(reader_config, kind, file).await {
            Ok(output) => read = Some(output),
            Err(problem) => run.problems.push(problem),
        }
    }

    let mut opinion: Option<SecondOpinion> = None;
    if let Some(classifier) = &config.classifier {
        let transcript = read.as_ref().map(|output: &reader::ReaderOutput| output.transcript.as_str());
        match jev::input_for(classifier.route, kind, file, transcript) {
            Some(input) => match jev::second_opinion(classifier, input).await {
                Ok(answer) => {
                    run.classifier_model = Some(answer.source.clone());
                    opinion = Some(answer);
                }
                Err(problem) => run.problems.push(problem),
            },
            None => run.problems.push(
                "Jev skipped: this route takes text only and the reader gave no transcript".to_owned(),
            ),
        }
    }

    run.reading = match (read, opinion) {
        (Some(output), Some(opinion)) => {
            let mut reading = output.reading;
            fold_second_opinion(&mut reading, &opinion);
            Some(reading)
        }
        (Some(output), None) => Some(output.reading),
        (None, Some(opinion)) => Some(reading_from_opinion(&opinion)),
        (None, None) => None,
    };
    run
}

async fn call_reader(
    config: &config::ReaderConfig,
    kind: UploadKind,
    file: &[u8],
) -> Result<reader::ReaderOutput, String> {
    let call = reader::build(config, kind, file);
    let mut headers = http::HeaderMap::new();
    for (name, value) in call.headers {
        let value = http::HeaderValue::from_str(&value)
            .map_err(|_| format!("reader header {name} is not a valid header value"))?;
        headers.insert(name, value);
    }
    let body = serde_json::to_vec(&call.body).map_err(|_| "reader request is not JSON".to_owned())?;
    let (status, _, reply) =
        transport::send(http::Method::POST, &call.url, headers, Bytes::from(body), READER_TIMEOUT)
            .await
            .map_err(|failure| format!("reader {}: {failure}", config.endpoint.model))?;
    reader_reply(config, status, &reply)
}

fn reader_reply(config: &config::ReaderConfig, status: u16, reply: &[u8]) -> Result<reader::ReaderOutput, String> {
    if !(200..300).contains(&status) {
        let detail: String = String::from_utf8_lossy(reply).chars().take(300).collect();
        return Err(format!("reader {} returned HTTP {status}: {detail}", config.endpoint.model));
    }
    reader::parse(config.shape, reply)
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::{ClassifierConfig, ClassifierRoute, Endpoint, ReaderConfig, ReaderShape};

    fn endpoint(model: &str) -> Endpoint {
        Endpoint { base_url: "https://ai.test".into(), api_key: "k".into(), model: model.into() }
    }

    fn reader() -> ReaderConfig {
        ReaderConfig { shape: ReaderShape::OpenAi, endpoint: endpoint("reader-1") }
    }

    #[test]
    fn only_a_2xx_reply_is_read() {
        let answer = serde_json::json!({
            "is_form_2303": true, "document_confidence": 0.9, "tin": null, "branch_code": null,
            "registered_name": null, "rdo_code": null, "registered_address": null,
            "tampering_signs": [], "transcript": "COR"
        });
        let body = serde_json::json!({"choices": [{"message": {"content": answer.to_string()}}]}).to_string();
        let output = reader_reply(&reader(), 200, body.as_bytes()).expect("read");
        assert_eq!(output.transcript, "COR");
        let error = reader_reply(&reader(), 503, b"overloaded").unwrap_err();
        assert_eq!(error, "reader reader-1 returned HTTP 503: overloaded");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn each_failed_or_skipped_model_is_a_problem() {
        let both = AssessmentConfig {
            reader: Some(reader()),
            classifier: Some(ClassifierConfig { route: ClassifierRoute::Decisions, endpoint: endpoint("jev") }),
        };
        let run = read_and_classify(&both, UploadKind::Pdf, b"%PDF").await;
        assert!(run.reading.is_none());
        assert_eq!(run.reader_model.as_deref(), Some("reader-1"));
        assert_eq!(run.classifier_model, None);
        assert_eq!(run.problems.len(), 2, "{:?}", run.problems);
        assert_eq!(run.problems[0], "reader reader-1: AI requests need the Spin runtime");
        assert!(run.problems[1].starts_with("Jev skipped"), "{:?}", run.problems);
    }
}
