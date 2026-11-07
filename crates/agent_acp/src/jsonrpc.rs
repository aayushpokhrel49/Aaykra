use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MessageId {
    Number(u64),
    String(String),
}

impl MessageId {
    pub fn to_value(&self) -> Value {
        match self {
            Self::Number(number) => (*number).into(),
            Self::String(string) => Value::from(string.clone()),
        }
    }

    pub fn from_value(value: &Value) -> Option<Self> {
        if let Some(number) = value.as_u64() {
            Some(Self::Number(number))
        } else if let Some(string) = value.as_str() {
            Some(Self::String(string.to_string()))
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResponseOutcome {
    Success(Value),
    Error { code: i64, message: String },
}

impl ResponseOutcome {
    pub fn to_value(&self) -> Value {
        match self {
            Self::Success(result) => result.clone(),
            Self::Error { code, message } => serde_json::json!({
                "code": code,
                "message": message,
            }),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum IncomingMessage {
    Request {
        id: MessageId,
        method: String,
        params: Value,
    },
    Notification {
        method: String,
        params: Value,
    },
    Response {
        id: MessageId,
        outcome: ResponseOutcome,
    },
    Malformed(String),
}

impl IncomingMessage {
    pub fn try_parse(line: &str) -> anyhow::Result<Option<Self>> {
        let line = line.trim();
        if line.is_empty() {
            return Ok(None);
        }

        let value: Value = serde_json::from_str(line).inspect_err(|error| {
            log::warn!("discarding non-JSON line from agent process: {error} ({line})");
        })?;

        match Self::from_value(value) {
            Some(Self::Malformed(line)) => anyhow::bail!("malformed JSON-RPC line: {line}"),
            message => Ok(message),
        }
    }

    pub fn from_value(value: Value) -> Option<Self> {
        let method = value.get("method").and_then(Value::as_str).map(String::from);
        let id = value
            .get("id")
            .filter(|id| !id.is_null())
            .and_then(MessageId::from_value);

        match method {
            Some(method) => {
                let params = value.get("params").cloned().unwrap_or_default();
                match id {
                    Some(id) => Some(Self::Request { id, method, params }),
                    None => Some(Self::Notification { method, params }),
                }
            }
            None => {
                let Some(id) = id else {
                    return Some(Self::Malformed(
                        "message has neither method nor id".to_string(),
                    ));
                };

                if let Some(result) = value.get("result") {
                    Some(Self::Response {
                        id,
                        outcome: ResponseOutcome::Success(result.clone()),
                    })
                } else if let Some(error) = value.get("error") {
                    Some(Self::Response {
                        id,
                        outcome: ResponseOutcome::Error {
                            code: error
                                .get("code")
                                .and_then(Value::as_i64)
                                .unwrap_or(-32000),
                            message: error
                                .get("message")
                                .and_then(Value::as_str)
                                .unwrap_or("agent did not provide an error message")
                                .to_string(),
                        },
                    })
                } else {
                    Some(Self::Malformed("response has neither result nor error".into()))
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum OutgoingMessage {
    Request {
        id: u64,
        method: String,
        params: Value,
    },
    Response {
        id: MessageId,
        outcome: ResponseOutcome,
    },
    Notification {
        method: String,
        params: Value,
    },
}

impl OutgoingMessage {
    pub fn write_to(&self, output: &mut Vec<u8>) {
        let value = serde_json::json!(match self {
            Self::Request { id, method, params } => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method,
                "params": params,
            }),
            Self::Response { id, outcome } if matches!(outcome, ResponseOutcome::Success(_)) => {
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id.to_value(),
                    "result": outcome.to_value(),
                })
            }
            Self::Response { id, outcome } => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id.to_value(),
                "error": outcome.to_value(),
            }),
            Self::Notification { method, params } => serde_json::json!({
                "jsonrpc": "2.0",
                "method": method,
                "params": params,
            }),
        });

        serde_json::to_writer(&mut *output, &value)
            .expect("JSON-RPC messages are always serializable");
        output.push(b'\n');
    }
}

#[cfg(test)]
pub(crate) mod test_util {
    use super::*;

    pub fn write_request(message: &OutgoingMessage) -> String {
        let mut output = Vec::new();
        message.write_to(&mut output);
        String::from_utf8(output).unwrap()
    }

    pub fn response_value(result: Value) -> Value {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 7,
            "result": result,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::*;
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_request_with_object_params() {
        let line = r#"{"jsonrpc":"2.0","id":3,"method":"session/prompt","params":{"sessionId":"s1"}}"#;
        let IncomingMessage::Request {
            id,
            method,
            params,
        } = IncomingMessage::try_parse(line).unwrap().unwrap()
        else {
            panic!("expected a request");
        };
        assert_eq!(id, MessageId::Number(3));
        assert_eq!(method, "session/prompt");
        assert_eq!(params["sessionId"], "s1");
    }

    #[test]
    fn parses_response_with_result() {
        let line = r#"{"jsonrpc":"2.0","id":"abc-1","result":{"protocolVersion":1}}"#;
        let IncomingMessage::Response { id, outcome } =
            IncomingMessage::try_parse(line).unwrap().unwrap()
        else {
            panic!("expected a response");
        };
        assert_eq!(id, MessageId::String("abc-1".into()));
        assert_eq!(outcome, ResponseOutcome::Success(json!({"protocolVersion": 1})));
    }

    #[test]
    fn parses_response_with_error() {
        let line = r#"{"jsonrpc":"2.0","id":9,"error":{"code":-32000,"message":"nope"}}"#;
        let IncomingMessage::Response { id, outcome } =
            IncomingMessage::try_parse(line).unwrap().unwrap()
        else {
            panic!("expected a response");
        };
        assert_eq!(id, MessageId::Number(9));
        assert_eq!(
            outcome,
            ResponseOutcome::Error {
                code: -32000,
                message: "nope".into()
            }
        );
    }

    #[test]
    fn parses_notification_without_id() {
        let line = r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s"}}"#;
        let IncomingMessage::Notification { method, params } =
            IncomingMessage::try_parse(line).unwrap().unwrap()
        else {
            panic!("expected a notification");
        };
        assert_eq!(method, "session/update");
        assert_eq!(params["sessionId"], "s");
    }

    #[test]
    fn rejects_malformed_json() {
        assert!(IncomingMessage::try_parse("this is not json").is_err());
    }

    #[test]
    fn rejects_message_without_method_or_id() {
        let result = IncomingMessage::try_parse(r#"{"jsonrpc":"2.0"}"#).unwrap().unwrap();
        assert!(matches!(result, IncomingMessage::Malformed(_)));
    }

    #[test]
    fn serializes_request() {
        let line = write_request(&OutgoingMessage::Request {
            id: 1,
            method: "initialize".into(),
            params: json!({"protocolVersion": 1}),
        });

        assert!(line.contains(r#""method":"initialize""#));
        assert!(line.contains(r#""id":1"#));
    }

    #[test]
    fn serializes_response_without_error_field_on_success() {
        let line = write_request(&OutgoingMessage::Response {
            id: MessageId::Number(4),
            outcome: ResponseOutcome::Success(json!({"content": "ok"})),
        });

        assert!(line.contains(r#""result""#));
        assert!(!line.contains(r#""error""#));
    }
}
