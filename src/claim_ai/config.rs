//! Which models read and classify a claim file, and how to reach them.
//!
//! One gateway (for example Merge: `AI_GATEWAY_BASE_URL=https://api-gateway.merge.dev`,
//! `AI_GATEWAY_API_KEY=mg_...`) serves both roles. Each role can override the
//! base URL, key and model, so the reader can be Anthropic direct while Jev
//! goes to TypeSafe, or either can run alone.
//!
//! | Variable | Meaning |
//! | --- | --- |
//! | `AI_READER_SHAPE` | `anthropic` (default), `openai`, `gemini`, or `off` |
//! | `AI_READER_BASE_URL`, `AI_READER_API_KEY`, `AI_READER_MODEL` | per-role overrides |
//! | `AI_CLASSIFIER` | `jev` (default when a key resolves) or `off` |
//! | `AI_CLASSIFIER_ROUTE` | `systemone` (TypeSafe, takes photos) or `decisions` (gateway, text only); defaults by host |
//! | `AI_CLASSIFIER_BASE_URL`, `AI_CLASSIFIER_API_KEY`, `AI_CLASSIFIER_MODEL` | per-role overrides |

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReaderShape {
    /// `POST /v1/messages`.
    Anthropic,
    /// `POST /v1/chat/completions`.
    OpenAi,
    /// `POST /v1beta/models/{model}:generateContent`. Merge has no such route.
    Gemini,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
    /// Origin or path prefix, without a trailing `/` or `/v1`.
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReaderConfig {
    pub shape: ReaderShape,
    pub endpoint: Endpoint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassifierRoute {
    /// TypeSafe `POST /v1/systemone`: text and PNG/JPEG photos.
    SystemOne,
    /// A gateway's `POST /v1/decisions` with the System One body: text only.
    Decisions,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassifierConfig {
    pub route: ClassifierRoute,
    pub endpoint: Endpoint,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AssessmentConfig {
    pub reader: Option<ReaderConfig>,
    pub classifier: Option<ClassifierConfig>,
}

impl AssessmentConfig {
    pub fn is_enabled(&self) -> bool {
        self.reader.is_some() || self.classifier.is_some()
    }
}

const TYPESAFE_BASE_URL: &str = "https://api.typesafe.ai";

pub fn resolve(lookup: impl Fn(&str) -> Option<String>) -> Result<AssessmentConfig, String> {
    let get = |name: &str| lookup(name).map(|value| value.trim().to_owned()).filter(|v| !v.is_empty());
    let gateway_base = get("AI_GATEWAY_BASE_URL").map(|url| normalize_base(&url));
    let gateway_key = get("AI_GATEWAY_API_KEY");

    let reader = match get("AI_READER_SHAPE").as_deref().unwrap_or("anthropic") {
        "off" => None,
        shape => {
            let shape = match shape {
                "anthropic" => ReaderShape::Anthropic,
                "openai" => ReaderShape::OpenAi,
                "gemini" => ReaderShape::Gemini,
                other => return Err(format!("AI_READER_SHAPE {other:?} is not anthropic, openai, gemini or off")),
            };
            let (base_url, via_gateway) = match get("AI_READER_BASE_URL") {
                Some(url) => (normalize_base(&url), false),
                None => match &gateway_base {
                    Some(url) => (url.clone(), true),
                    None => (default_reader_base(shape).to_owned(), false),
                },
            };
            match get("AI_READER_API_KEY").or_else(|| gateway_key.clone()) {
                None => None,
                Some(api_key) => {
                    let model = match get("AI_READER_MODEL") {
                        Some(model) => model,
                        None => default_reader_model(shape, via_gateway).ok_or_else(|| {
                            "AI_READER_MODEL is required for the openai shape".to_owned()
                        })?,
                    };
                    Some(ReaderConfig { shape, endpoint: Endpoint { base_url, api_key, model } })
                }
            }
        }
    };

    let classifier = match get("AI_CLASSIFIER").as_deref().unwrap_or("jev") {
        "off" => None,
        "jev" => {
            let (base_url, via_gateway) = match get("AI_CLASSIFIER_BASE_URL") {
                Some(url) => (normalize_base(&url), false),
                None => match &gateway_base {
                    Some(url) => (url.clone(), true),
                    None => (TYPESAFE_BASE_URL.to_owned(), false),
                },
            };
            match get("AI_CLASSIFIER_API_KEY").or_else(|| gateway_key.clone()) {
                None => None,
                Some(api_key) => {
                    let route = match get("AI_CLASSIFIER_ROUTE").as_deref() {
                        Some("systemone") => ClassifierRoute::SystemOne,
                        Some("decisions") => ClassifierRoute::Decisions,
                        Some(other) => {
                            return Err(format!("AI_CLASSIFIER_ROUTE {other:?} is not systemone or decisions"));
                        }
                        None if base_url.contains("typesafe.ai") => ClassifierRoute::SystemOne,
                        None => ClassifierRoute::Decisions,
                    };
                    let model = get("AI_CLASSIFIER_MODEL").unwrap_or_else(|| {
                        if via_gateway { "typesafe/jev-1.13" } else { "jev-latest" }.to_owned()
                    });
                    Some(ClassifierConfig { route, endpoint: Endpoint { base_url, api_key, model } })
                }
            }
        }
        other => return Err(format!("AI_CLASSIFIER {other:?} is not jev or off")),
    };

    Ok(AssessmentConfig { reader, classifier })
}

/// Gateways document their base with `/v1`; every route here adds its own.
fn normalize_base(url: &str) -> String {
    let url = url.trim().trim_end_matches('/');
    url.strip_suffix("/v1").unwrap_or(url).to_owned()
}

fn default_reader_base(shape: ReaderShape) -> &'static str {
    match shape {
        ReaderShape::Anthropic => "https://api.anthropic.com",
        ReaderShape::OpenAi => "https://api.openai.com",
        ReaderShape::Gemini => "https://generativelanguage.googleapis.com",
    }
}

fn default_reader_model(shape: ReaderShape, via_gateway: bool) -> Option<String> {
    let model = match (shape, via_gateway) {
        (ReaderShape::Anthropic, true) => "anthropic/claude-haiku-5-5",
        (ReaderShape::Anthropic, false) => "claude-haiku-5-5",
        (ReaderShape::Gemini, true) => "google/gemini-3.8-flash",
        (ReaderShape::Gemini, false) => "gemini-3.8-flash",
        (ReaderShape::OpenAi, _) => return None,
    };
    Some(model.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn resolve_with(pairs: &[(&str, &str)]) -> Result<AssessmentConfig, String> {
        let vars: HashMap<String, String> =
            pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        resolve(|name| vars.get(name).cloned())
    }

    #[test]
    fn nothing_configured_turns_assessment_off() {
        let config = resolve_with(&[]).unwrap();
        assert!(!config.is_enabled());
    }

    #[test]
    fn one_gateway_key_serves_both_roles() {
        let config = resolve_with(&[
            ("AI_GATEWAY_BASE_URL", "https://api-gateway.merge.dev/v1/"),
            ("AI_GATEWAY_API_KEY", "mg_test"),
        ])
        .unwrap();
        let reader = config.reader.unwrap();
        assert_eq!(reader.shape, ReaderShape::Anthropic);
        assert_eq!(reader.endpoint.base_url, "https://api-gateway.merge.dev");
        assert_eq!(reader.endpoint.model, "anthropic/claude-haiku-5-5");
        let classifier = config.classifier.unwrap();
        assert_eq!(classifier.route, ClassifierRoute::Decisions);
        assert_eq!(classifier.endpoint.model, "typesafe/jev-1.13");
        assert_eq!(classifier.endpoint.api_key, "mg_test");
    }

    #[test]
    fn each_role_can_have_its_own_key_and_host() {
        let config = resolve_with(&[
            ("AI_GATEWAY_BASE_URL", "https://api-gateway.merge.dev"),
            ("AI_GATEWAY_API_KEY", "mg_test"),
            ("AI_CLASSIFIER_BASE_URL", "https://api.typesafe.ai"),
            ("AI_CLASSIFIER_API_KEY", "ts_test"),
        ])
        .unwrap();
        let classifier = config.classifier.unwrap();
        assert_eq!(classifier.route, ClassifierRoute::SystemOne);
        assert_eq!(classifier.endpoint.api_key, "ts_test");
        assert_eq!(classifier.endpoint.model, "jev-latest");
        assert_eq!(config.reader.unwrap().endpoint.api_key, "mg_test");
    }

    #[test]
    fn either_role_runs_alone() {
        let jev_only = resolve_with(&[("AI_READER_SHAPE", "off"), ("AI_CLASSIFIER_API_KEY", "ts")]).unwrap();
        assert!(jev_only.is_enabled());
        assert!(jev_only.reader.is_none());
        assert_eq!(jev_only.classifier.unwrap().endpoint.base_url, "https://api.typesafe.ai");

        let anthropic_only = resolve_with(&[("AI_CLASSIFIER", "off"), ("AI_READER_API_KEY", "sk-ant")]).unwrap();
        assert!(anthropic_only.is_enabled());
        assert!(anthropic_only.classifier.is_none());
        let reader = anthropic_only.reader.unwrap();
        assert_eq!(reader.endpoint.base_url, "https://api.anthropic.com");
        assert_eq!(reader.endpoint.model, "claude-haiku-5-5");
    }

    #[test]
    fn bad_values_are_errors_not_silent_defaults() {
        assert!(resolve_with(&[("AI_READER_SHAPE", "claude")]).is_err());
        assert!(resolve_with(&[("AI_CLASSIFIER", "maybe")]).is_err());
        assert!(resolve_with(&[("AI_READER_SHAPE", "openai"), ("AI_READER_API_KEY", "sk")]).is_err());
    }
}
