//! Azure OpenAI's deployment-scoped audio API, shared by CLI and Agent STT.

use async_trait::async_trait;
use reqwest::{header::HeaderValue, multipart::Form, Client, RequestBuilder, Url};

use super::{
    parse_response, request_form, SpeechToText, SttConfig, SttError, SttMode, SttRequest,
    SttResponse,
};
use crate::agent::media::util;
use std::time::Duration;

pub(crate) const DEFAULT_API_VERSION: &str = "2024-10-21";
pub(crate) const DEFAULT_KEY_ENV: &str = "AZURE_OPENAI_API_KEY";

pub(crate) fn resource_url(base_url: Option<&str>, deployment: &str) -> Result<Url, String> {
    let url = base_url.map(str::trim).filter(|url| !url.is_empty()).ok_or(
        "Azure STT requires --base-url set to the Azure OpenAI resource root (e.g. https://<resource>.openai.azure.com/)",
    )?;
    let mut url = Url::parse(url).map_err(|_| "invalid Azure OpenAI resource URL")?;
    let azure_host = url.host_str().is_some_and(|host| {
        [".openai.azure.com", ".openai.azure.us", ".openai.azure.cn"]
            .iter()
            .any(|suffix| {
                host.strip_suffix(*suffix)
                    .is_some_and(|prefix| !prefix.is_empty())
            })
    });
    if url.scheme() != "https"
        || !azure_host
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.fragment().is_some()
    {
        return Err("Azure STT requires an HTTPS Azure OpenAI resource root URL, not a deployment or API path".into());
    }

    if deployment.is_empty()
        || matches!(deployment, "." | "..")
        || !deployment
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(
            "Azure STT --model must be your deployment name (letters, digits, '-', '_' or '.')"
                .into(),
        );
    }

    // Setup stores an explicit --api-version in base_url, as for chat.
    let version = if url.query().is_some() {
        let mut pairs = url.query_pairs();
        let Some((name, version)) = pairs.next() else {
            return Err("Azure STT resource URL query must contain only api-version".into());
        };
        if name != "api-version" || pairs.next().is_some() {
            return Err("Azure STT resource URL query must contain only api-version".into());
        }
        let date = version.strip_suffix("-preview").unwrap_or(&version);
        if date.len() != 10 || chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
            return Err("Azure STT --api-version must be YYYY-MM-DD or YYYY-MM-DD-preview".into());
        }
        version.into_owned()
    } else {
        DEFAULT_API_VERSION.into()
    };
    url.set_query(None);
    url.query_pairs_mut().append_pair("api-version", &version);
    Ok(url)
}

pub(crate) fn key_environment<'a>(
    credential: Option<&str>,
    env: Option<&'a str>,
) -> Option<&'a str> {
    env.or_else(|| credential.is_none().then_some(DEFAULT_KEY_ENV))
}

pub(crate) fn resolve_key(
    credential: Option<&str>,
    env: Option<&str>,
) -> Result<Option<HeaderValue>, String> {
    let key = crate::agent::llm::construction::resolve_process_api_key(
        credential,
        key_environment(credential, env),
    )
    .map_err(|_| "could not read the configured Azure STT credential")?;
    key.map(|value| {
        let mut header = HeaderValue::try_from(value).map_err(|_| "invalid Azure STT API key")?;
        header.set_sensitive(true);
        Ok(header)
    })
    .transpose()
}

pub(crate) struct AzureStt {
    resource: Url,
    deployment: String,
    api_key: Option<HeaderValue>,
    timeout: Duration,
    default_response_format: String,
}

impl AzureStt {
    pub(crate) fn from_config(cfg: &SttConfig) -> Result<Self, String> {
        Ok(Self {
            resource: resource_url(cfg.base_url.as_deref(), &cfg.model)?,
            deployment: cfg.model.clone(),
            api_key: resolve_key(
                cfg.api_key_credential.as_deref(),
                cfg.api_key_env.as_deref(),
            )?,
            timeout: Duration::from_secs(cfg.request_timeout),
            default_response_format: cfg.default_response_format.clone(),
        })
    }

    fn endpoint(&self, mode: SttMode) -> Url {
        let mut url = self.resource.clone();
        url.set_path(&format!(
            "/openai/deployments/{}/{}",
            self.deployment,
            mode.path()
        ));
        url
    }

    fn request(client: &Client, url: Url, form: Form, key: HeaderValue) -> RequestBuilder {
        // Do not forward extra_headers carried over from a different provider.
        client.post(url).header("api-key", key).multipart(form)
    }

    async fn decode(
        &self,
        response: reqwest::Response,
        format: &str,
        mode: SttMode,
    ) -> Result<SttResponse, SttError> {
        if !response.status().is_success() {
            // Don't surface error bodies, which may echo sensitive request data.
            let status = response.status().as_u16();
            return Err(match status {
                401 | 403 => SttError::Auth,
                429 => SttError::RateLimited { retry_after_ms: 1000 },
                _ => SttError::Provider {
                    status,
                    message: match (mode, status) {
                        (_, 300..=399) => "Azure STT does not follow redirects.",
                        (SttMode::Translate, 400 | 404 | 422) => "Azure audio translation requires a Whisper deployment that supports translations. For a GPT transcription deployment, use `yu model transcribe`.",
                        (_, 400 | 404 | 422) => "Check the Azure deployment name, API version, audio file and response format.",
                        _ => "Azure STT request failed.",
                    }.into(),
                },
            });
        }
        let bytes = util::read_bytes_capped(response, util::MAX_TEXT_BODY_BYTES, "azure stt")
            .await
            .map_err(|e| SttError::Transport(e.to_string()))?;
        let result = parse_response(&bytes, format, &self.deployment)?;
        if !matches!(format, "text" | "srt" | "vtt") && !result.raw["text"].is_string() {
            return Err(SttError::Parse("Azure STT response is missing text".into()));
        }
        Ok(result)
    }
}

#[async_trait]
impl SpeechToText for AzureStt {
    fn name(&self) -> &str {
        "azure"
    }

    fn model(&self) -> &str {
        &self.deployment
    }

    fn is_configured(&self) -> bool {
        self.api_key.is_some()
    }

    async fn transcribe(&self, request: SttRequest) -> Result<SttResponse, SttError> {
        let key = self.api_key.clone().ok_or(SttError::Auth)?;
        let format = request
            .response_format
            .clone()
            .unwrap_or_else(|| self.default_response_format.clone());
        let mode = request.mode;
        let form = request_form(request, None, &format)?;
        let url = self.endpoint(mode);
        let client = util::build_origin_bound_client(&url, self.timeout)
            .await
            .map_err(|e| SttError::Transport(e.to_string()))?;
        let response = Self::request(&client, url, form, key)
            .send()
            .await
            .map_err(|e| SttError::Transport(e.without_url().to_string()))?;
        self.decode(response, &format, mode).await
    }
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test/unit/model/tasks/stt/azure.rs"
    ));
}
