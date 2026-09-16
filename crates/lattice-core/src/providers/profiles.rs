//! Remote provider profiles (0.11): the non-secret reference a user
//! configures for a remote OpenAI-compatible endpoint, the validation that
//! keeps its endpoint an explicit HTTPS destination, and
//! [`prepare_remote_target`], the one use case that turns a profile plus a
//! canonical [`ChatRequest`] into a dispatchable target. Persistence lives
//! in `storage::provider_profiles`, mirroring `credentials`'s own
//! domain/storage split. A profile never carries a secret: `credential_id`
//! is a 0.10 reference, resolved only inside `prepare_remote_target` at
//! dispatch time and never cached.

use super::completion::{check_prompt_bound, ChatRequest, CompletionTarget};
use super::openai_compatible::BearerToken;
use crate::error::AppError;
use serde::{Deserialize, Serialize};

pub const LIST_PROVIDER_PROFILES_COMMAND: &str = "list_provider_profiles";
pub const CREATE_PROVIDER_PROFILE_COMMAND: &str = "create_provider_profile";
pub const UPDATE_PROVIDER_PROFILE_COMMAND: &str = "update_provider_profile";
pub const DELETE_PROVIDER_PROFILE_COMMAND: &str = "delete_provider_profile";
pub const BIND_PROVIDER_CREDENTIAL_COMMAND: &str = "bind_provider_credential";
pub const GRANT_PROVIDER_CONSENT_COMMAND: &str = "grant_provider_consent";
pub const REVOKE_PROVIDER_CONSENT_COMMAND: &str = "revoke_provider_consent";

/// Provider key recorded on assistant messages generated through a remote
/// profile, alongside 0.9's `LOCAL_PROVIDER_KEY`. Names the protocol
/// family, never the endpoint or credential (conversations spec: provenance
/// never contains endpoint or credential data).
pub const REMOTE_PROVIDER_KEY: &str = "remote-openai-compatible";

pub(crate) const MAX_PROVIDER_PROFILES: i64 = 50;
pub(crate) const MAX_PROFILE_LABEL_CHARS: usize = 120;
pub(crate) const MAX_MODEL_KEY_CHARS: usize = 200;
pub(crate) const MAX_ENDPOINT_BYTES: usize = 2048;
const MAX_CREDENTIAL_ID_CHARS: usize = 64;

/// Consent to send conversation context to exactly `endpoint`. Only ever
/// reported (and only ever honored) while it equals the profile's current
/// endpoint — see `storage::provider_profiles`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConsent {
    pub endpoint: String,
    pub granted_at_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfile {
    pub id: String,
    pub revision: u64,
    pub label: String,
    pub endpoint: String,
    pub model_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consent: Option<ProviderConsent>,
    pub created_at_unix_seconds: u64,
    pub updated_at_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProviderProfileRequest {
    pub label: String,
    pub endpoint: String,
    pub model_key: String,
    #[serde(default)]
    pub credential_id: Option<String>,
}

/// Deliberately carries no credential: binding is its own explicit action
/// (`BindProviderCredentialRequest`), so an endpoint change can never keep
/// sending the previously bound key implicitly.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProviderProfileRequest {
    pub id: String,
    pub expected_revision: u64,
    pub label: String,
    pub endpoint: String,
    pub model_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteProviderProfileRequest {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BindProviderCredentialRequest {
    pub id: String,
    pub expected_revision: u64,
    #[serde(default)]
    pub credential_id: Option<String>,
}

/// `endpoint` is the destination the disclosure the user accepted named;
/// consent is refused unless it still equals the profile's endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrantProviderConsentRequest {
    pub id: String,
    pub expected_revision: u64,
    pub endpoint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevokeProviderConsentRequest {
    pub id: String,
    pub expected_revision: u64,
}

pub fn new_provider_profile_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub(crate) fn validate_profile_label(label: &str) -> Result<String, AppError> {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        return Err(AppError::provider_invalid("Give the provider a label."));
    }
    if trimmed.chars().count() > MAX_PROFILE_LABEL_CHARS {
        return Err(AppError::provider_invalid(
            "The provider label is too long.",
        ));
    }
    Ok(trimmed.to_string())
}

pub(crate) fn validate_model_key(model_key: &str) -> Result<String, AppError> {
    let trimmed = model_key.trim();
    if trimmed.is_empty() {
        return Err(AppError::provider_invalid(
            "Enter the provider's model name.",
        ));
    }
    if trimmed.chars().count() > MAX_MODEL_KEY_CHARS {
        return Err(AppError::provider_invalid("The model name is too long."));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(AppError::provider_invalid(
            "The model name cannot contain control characters.",
        ));
    }
    Ok(trimmed.to_string())
}

pub(crate) fn validate_credential_id(
    credential_id: Option<&str>,
) -> Result<Option<String>, AppError> {
    let Some(credential_id) = credential_id else {
        return Ok(None);
    };
    let trimmed = credential_id.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_CREDENTIAL_ID_CHARS {
        return Err(AppError::provider_invalid(
            "That credential no longer exists.",
        ));
    }
    Ok(Some(trimmed.to_string()))
}

/// Accepts only an absolute `https` base URL with a host and without user
/// information, query or fragment, and returns it normalized: lowercase
/// scheme and authority, no trailing `/`. The normalized form is what
/// consent is bound to, so two spellings of one destination compare equal.
pub(crate) fn validate_endpoint(endpoint: &str) -> Result<String, AppError> {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return Err(AppError::provider_invalid(
            "Enter the provider's HTTPS endpoint.",
        ));
    }
    if trimmed.len() > MAX_ENDPOINT_BYTES {
        return Err(AppError::provider_invalid("The endpoint is too long."));
    }
    if !trimmed.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err(AppError::provider_invalid(
            "The endpoint must be a plain URL without spaces.",
        ));
    }

    let Some((scheme, rest)) = trimmed.split_once("://") else {
        return Err(AppError::provider_invalid(
            "Remote endpoints must start with https://.",
        ));
    };
    if !scheme.eq_ignore_ascii_case("https") {
        return Err(AppError::provider_invalid(
            "Remote endpoints must start with https://.",
        ));
    }
    if rest.contains(['?', '#', '@', '\\']) {
        return Err(AppError::provider_invalid(
            "The endpoint cannot contain credentials, a query, or a fragment.",
        ));
    }

    let (authority, path) = match rest.find('/') {
        Some(index) => rest.split_at(index),
        None => (rest, ""),
    };
    let authority = authority.to_ascii_lowercase();
    validate_authority(&authority)?;

    let normalized = format!("https://{authority}{}", path.trim_end_matches('/'));
    let uri = ureq::http::Uri::try_from(normalized.as_str())
        .map_err(|_| AppError::provider_invalid("The endpoint is not a valid URL."))?;
    if uri.scheme_str() != Some("https") || uri.host().is_none_or(str::is_empty) {
        return Err(AppError::provider_invalid(
            "The endpoint is not a valid URL.",
        ));
    }
    Ok(normalized)
}

fn validate_authority(authority: &str) -> Result<(), AppError> {
    const MISSING_HOST: AppError = AppError::provider_invalid("The endpoint needs a host name.");
    const BAD_PORT: AppError = AppError::provider_invalid("The endpoint port is not valid.");

    let (host, port) = if let Some(bracketed) = authority.strip_prefix('[') {
        let Some((address, after)) = bracketed.split_once(']') else {
            return Err(MISSING_HOST);
        };
        if address.is_empty()
            || !address
                .chars()
                .all(|character| character.is_ascii_hexdigit() || matches!(character, ':' | '.'))
        {
            return Err(MISSING_HOST);
        }
        match after {
            "" => (address, None),
            _ => match after.strip_prefix(':') {
                Some(port) => (address, Some(port)),
                None => return Err(MISSING_HOST),
            },
        }
    } else {
        match authority.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        }
    };

    if host.is_empty()
        || !host.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '.' | ':')
        })
    {
        return Err(MISSING_HOST);
    }

    if let Some(port) = port {
        let valid = !port.is_empty()
            && port.bytes().all(|byte| byte.is_ascii_digit())
            && port.parse::<u16>().is_ok_and(|value| value >= 1);
        if !valid {
            return Err(BAD_PORT);
        }
    }
    Ok(())
}

/// The remote authorization use case: refuses before any network call
/// unless the prompt is within bounds, the request names the profile's
/// configured model, and the profile holds consent valid for its current
/// endpoint; only then resolves the bound credential (if any) through
/// `resolve_secret` — in production `CredentialStore::resolve_secret` — and
/// builds the target. Resolver errors (`credential.not_found`,
/// `credential.unavailable`, `credential.unsupported`) pass through
/// unchanged so the UI can show the credential's own safe state.
pub fn prepare_remote_target(
    profile: &ProviderProfile,
    request: &ChatRequest,
    resolve_secret: impl FnOnce(&str) -> Result<Vec<u8>, AppError>,
) -> Result<CompletionTarget, AppError> {
    check_prompt_bound(request)?;

    if request.model_key != profile.model_key {
        return Err(AppError::provider_invalid(
            "The request does not use this provider's configured model.",
        ));
    }

    let consented = profile
        .consent
        .as_ref()
        .is_some_and(|consent| consent.endpoint == profile.endpoint);
    if !consented {
        return Err(AppError::provider_consent_required(
            "Approve sending this conversation to the provider first.",
        ));
    }

    let bearer = match profile.credential_id.as_deref() {
        Some(credential_id) => Some(BearerToken::from_secret(resolve_secret(credential_id)?)?),
        None => None,
    };

    Ok(CompletionTarget::remote(&profile.endpoint, bearer))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::completion::{ChatMessage, ChatRole, MAX_PROMPT_CHARS};
    use std::error::Error;

    fn profile() -> ProviderProfile {
        ProviderProfile {
            id: "profile-1".to_string(),
            revision: 1,
            label: "Example".to_string(),
            endpoint: "https://api.example.com/v1".to_string(),
            model_key: "example-model".to_string(),
            credential_id: Some("credential-1".to_string()),
            consent: Some(ProviderConsent {
                endpoint: "https://api.example.com/v1".to_string(),
                granted_at_unix_seconds: 1,
            }),
            created_at_unix_seconds: 1,
            updated_at_unix_seconds: 1,
        }
    }

    fn request(model_key: &str, text: &str) -> ChatRequest {
        ChatRequest {
            model_key: model_key.to_string(),
            messages: vec![ChatMessage {
                role: ChatRole::User,
                text: text.to_string(),
            }],
        }
    }

    fn never_resolved(_: &str) -> Result<Vec<u8>, AppError> {
        Err(AppError::internal("the credential must not be resolved"))
    }

    #[test]
    fn accepts_and_normalizes_https_endpoints() -> Result<(), Box<dyn Error>> {
        for (input, expected) in [
            ("https://api.example.com/v1", "https://api.example.com/v1"),
            (
                "  https://api.example.com/v1/  ",
                "https://api.example.com/v1",
            ),
            ("HTTPS://API.Example.com/v1", "https://api.example.com/v1"),
            ("https://api.example.com", "https://api.example.com"),
            (
                "https://api.example.com:8443/openai/v1",
                "https://api.example.com:8443/openai/v1",
            ),
            ("https://127.0.0.1:9000/v1", "https://127.0.0.1:9000/v1"),
            ("https://[::1]:9000/v1", "https://[::1]:9000/v1"),
        ] {
            assert_eq!(validate_endpoint(input)?, expected, "input {input}");
        }
        Ok(())
    }

    #[test]
    fn refuses_endpoints_that_are_not_explicit_https_destinations() {
        for input in [
            "",
            "api.example.com/v1",
            "http://api.example.com/v1",
            "ftp://api.example.com",
            "https://",
            "https:///v1",
            "https://user:pass@api.example.com/v1",
            "https://api.example.com/v1?key=secret",
            "https://api.example.com/v1#fragment",
            "https://api.example.com/v 1",
            "https://api.example.com:0/v1",
            "https://api.example.com:99999/v1",
            "https://api.example.com:/v1",
            "https://api.ex_ample.com/v1",
            "https://[::1/v1",
            "https://api.exämple.com/v1",
        ] {
            let result = validate_endpoint(input);
            assert!(
                matches!(&result, Err(error) if error.code == "provider.invalid"),
                "expected refusal for {input:?}, got {result:?}"
            );
        }

        let oversized = format!("https://api.example.com/{}", "a".repeat(MAX_ENDPOINT_BYTES));
        assert!(validate_endpoint(&oversized).is_err());
    }

    #[test]
    fn validates_label_and_model_key() {
        assert!(validate_profile_label("  ").is_err());
        assert!(validate_profile_label(&"a".repeat(MAX_PROFILE_LABEL_CHARS + 1)).is_err());
        assert_eq!(
            validate_profile_label(" OpenAI ").ok().as_deref(),
            Some("OpenAI")
        );
        assert!(validate_model_key("").is_err());
        assert!(validate_model_key("model\nname").is_err());
        assert!(validate_model_key(&"m".repeat(MAX_MODEL_KEY_CHARS + 1)).is_err());
        assert_eq!(
            validate_model_key(" gpt-4o-mini ").ok().as_deref(),
            Some("gpt-4o-mini")
        );
        assert_eq!(validate_credential_id(None).ok(), Some(None));
        assert!(validate_credential_id(Some(" ")).is_err());
    }

    #[test]
    fn provider_profile_serializes_without_any_secret_shaped_field() -> Result<(), Box<dyn Error>> {
        let value = serde_json::to_value(profile())?;
        let keys: Vec<&str> = value
            .as_object()
            .ok_or("profile must serialize as an object")?
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            [
                "consent",
                "createdAtUnixSeconds",
                "credentialId",
                "endpoint",
                "id",
                "label",
                "modelKey",
                "revision",
                "updatedAtUnixSeconds"
            ]
        );
        Ok(())
    }

    #[test]
    fn prepares_a_remote_target_resolving_the_bound_credential() -> Result<(), Box<dyn Error>> {
        let mut resolved = Vec::new();
        let target = prepare_remote_target(&profile(), &request("example-model", "hi"), |id| {
            resolved.push(id.to_string());
            Ok(b"sk-test\n".to_vec())
        })?;

        assert_eq!(resolved, ["credential-1"]);
        assert_eq!(target.provider_key(), REMOTE_PROVIDER_KEY);
        assert!(!format!("{target:?}").contains("sk-test"));
        Ok(())
    }

    #[test]
    fn an_unbound_profile_dispatches_without_resolving_a_credential() -> Result<(), Box<dyn Error>>
    {
        let mut unbound = profile();
        unbound.credential_id = None;
        let target =
            prepare_remote_target(&unbound, &request("example-model", "hi"), never_resolved)?;
        assert_eq!(target.provider_key(), REMOTE_PROVIDER_KEY);
        Ok(())
    }

    #[test]
    fn refuses_a_model_other_than_the_profiles_before_resolving() -> Result<(), Box<dyn Error>> {
        let Err(error) = prepare_remote_target(&profile(), &request("other", "hi"), never_resolved)
        else {
            return Err("a mismatched model must be refused".into());
        };
        assert_eq!(error.code, "provider.invalid");
        Ok(())
    }

    #[test]
    fn refuses_missing_or_stale_consent_before_resolving() -> Result<(), Box<dyn Error>> {
        let mut without_consent = profile();
        without_consent.consent = None;
        let mut stale_consent = profile();
        stale_consent.endpoint = "https://other.example.com/v1".to_string();

        for candidate in [without_consent, stale_consent] {
            let Err(error) =
                prepare_remote_target(&candidate, &request("example-model", "hi"), never_resolved)
            else {
                return Err("a request without valid consent must be refused".into());
            };
            assert_eq!(error.code, "provider.consent_required");
        }
        Ok(())
    }

    #[test]
    fn refuses_an_oversized_prompt_before_resolving() -> Result<(), Box<dyn Error>> {
        let oversized = "a".repeat(MAX_PROMPT_CHARS + 1);
        let Err(error) = prepare_remote_target(
            &profile(),
            &request("example-model", &oversized),
            never_resolved,
        ) else {
            return Err("an oversized prompt must be refused".into());
        };
        assert_eq!(error.code, "chat.invalid");
        Ok(())
    }

    #[test]
    fn credential_resolution_errors_pass_through_unchanged() -> Result<(), Box<dyn Error>> {
        for resolver_error in [
            AppError::credential_not_found("That credential's secret is missing."),
            AppError::credential_unavailable("Unlock the keychain and try again."),
            AppError::credential_unsupported("Secure credential storage is not supported."),
        ] {
            let expected = resolver_error.clone();
            let Err(error) =
                prepare_remote_target(&profile(), &request("example-model", "hi"), |_| {
                    Err(resolver_error)
                })
            else {
                return Err("a credential error must refuse the request".into());
            };
            assert_eq!(error, expected);
        }
        Ok(())
    }

    #[test]
    fn refuses_secret_bytes_that_cannot_be_a_bearer_token() -> Result<(), Box<dyn Error>> {
        for secret in [
            b"".to_vec(),
            b"  \n".to_vec(),
            b"sk test".to_vec(),
            vec![0xff, 0x41],
        ] {
            let Err(error) =
                prepare_remote_target(&profile(), &request("example-model", "hi"), |_| Ok(secret))
            else {
                return Err("an unusable secret must be refused".into());
            };
            assert_eq!(error.code, "provider.invalid");
            assert!(!error.message.contains("sk"));
        }
        Ok(())
    }
}
