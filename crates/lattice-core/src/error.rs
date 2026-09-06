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
