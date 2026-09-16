use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize)]
#[serde(rename_all = "camelCase")]
#[error("{message}")]
pub struct AppError {
    pub code: &'static str,
    pub message: &'static str,
    pub recoverable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<&'static str>,
}

impl AppError {
    pub const fn internal(message: &'static str) -> Self {
        Self {
            code: "core.internal",
            message,
            recoverable: false,
            correlation_id: None,
        }
    }

    pub const fn unavailable(message: &'static str) -> Self {
        Self {
            code: "core.unavailable",
            message,
            recoverable: true,
            correlation_id: None,
        }
    }

    pub const fn invalid_settings(message: &'static str) -> Self {
        Self {
            code: "settings.invalid",
            message,
            recoverable: true,
            correlation_id: None,
        }
    }

    pub const fn settings_conflict(message: &'static str) -> Self {
        Self {
            code: "settings.conflict",
            message,
            recoverable: true,
            correlation_id: None,
        }
    }

    pub const fn invalid_runtime(message: &'static str) -> Self {
        Self {
            code: "runtime.invalid",
            message,
            recoverable: true,
            correlation_id: None,
        }
    }

    pub const fn runtime_conflict(message: &'static str) -> Self {
        Self {
            code: "runtime.conflict",
            message,
            recoverable: true,
            correlation_id: None,
        }
    }

    pub const fn storage_unavailable(message: &'static str) -> Self {
        Self {
            code: "storage.unavailable",
            message,
            recoverable: true,
            correlation_id: None,
        }
    }

    pub const fn migration_failed(message: &'static str) -> Self {
        Self {
            code: "storage.migration_failed",
            message,
            recoverable: true,
            correlation_id: None,
        }
    }

    pub const fn unsupported_schema(message: &'static str) -> Self {
        Self {
            code: "storage.unsupported_schema",
            message,
            recoverable: false,
            correlation_id: None,
        }
    }

    pub const fn chat_invalid(message: &'static str) -> Self {
        Self {
            code: "chat.invalid",
            message,
            recoverable: true,
            correlation_id: None,
        }
    }

    pub const fn chat_conflict(message: &'static str) -> Self {
        Self {
            code: "chat.conflict",
            message,
            recoverable: true,
            correlation_id: None,
        }
    }

    pub const fn chat_failed(message: &'static str) -> Self {
        Self {
            code: "chat.failed",
            message,
            recoverable: true,
            correlation_id: None,
        }
    }

    pub const fn conversation_not_found(message: &'static str) -> Self {
        Self {
            code: "conversation.not_found",
            message,
            recoverable: true,
            correlation_id: None,
        }
    }

    pub const fn conversation_conflict(message: &'static str) -> Self {
        Self {
            code: "conversation.conflict",
            message,
            recoverable: true,
            correlation_id: None,
        }
    }

    pub const fn credential_cancelled(message: &'static str) -> Self {
        Self {
            code: "credential.cancelled",
            message,
            recoverable: true,
            correlation_id: None,
        }
    }

    pub const fn credential_not_found(message: &'static str) -> Self {
        Self {
            code: "credential.not_found",
            message,
            recoverable: true,
            correlation_id: None,
        }
    }

    pub const fn credential_unavailable(message: &'static str) -> Self {
        Self {
            code: "credential.unavailable",
            message,
            recoverable: true,
            correlation_id: None,
        }
    }

    pub const fn credential_unsupported(message: &'static str) -> Self {
        Self {
            code: "credential.unsupported",
            message,
            recoverable: false,
            correlation_id: None,
        }
    }

    pub const fn provider_invalid(message: &'static str) -> Self {
        Self {
            code: "provider.invalid",
            message,
            recoverable: true,
            correlation_id: None,
        }
    }

    pub const fn provider_not_found(message: &'static str) -> Self {
        Self {
            code: "provider.not_found",
            message,
            recoverable: true,
            correlation_id: None,
        }
    }

    pub const fn provider_conflict(message: &'static str) -> Self {
        Self {
            code: "provider.conflict",
            message,
            recoverable: true,
            correlation_id: None,
        }
    }

    pub const fn provider_consent_required(message: &'static str) -> Self {
        Self {
            code: "provider.consent_required",
            message,
            recoverable: true,
            correlation_id: None,
        }
    }

    pub const fn provider_auth_failed(message: &'static str) -> Self {
        Self {
            code: "provider.auth_failed",
            message,
            recoverable: true,
            correlation_id: None,
        }
    }

    pub const fn provider_rate_limited(message: &'static str) -> Self {
        Self {
            code: "provider.rate_limited",
            message,
            recoverable: true,
            correlation_id: None,
        }
    }

    pub const fn provider_timeout(message: &'static str) -> Self {
        Self {
            code: "provider.timeout",
            message,
            recoverable: true,
            correlation_id: None,
        }
    }

    pub const fn provider_unavailable(message: &'static str) -> Self {
        Self {
            code: "provider.unavailable",
            message,
            recoverable: true,
            correlation_id: None,
        }
    }

    pub const fn provider_rejected(message: &'static str) -> Self {
        Self {
            code: "provider.rejected",
            message,
            recoverable: true,
            correlation_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AppError;

    #[test]
    fn serializes_structured_errors_for_ipc() {
        let error = AppError::internal("Something failed.");
        let json = serde_json::to_value(error).unwrap_or_else(|serde_error| {
            serde_json::json!({
                "code": "test.serialization",
                "message": serde_error.to_string(),
                "recoverable": false
            })
        });

        assert_eq!(json["code"], "core.internal");
        assert_eq!(json["message"], "Something failed.");
        assert_eq!(json["recoverable"], false);
        assert!(json.get("correlationId").is_none());
    }
}
