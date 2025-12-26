use serde_json::Value;
use tokio_postgres::types::Json;

/// Convert an optional JSON value into a Postgres-compatible wrapper.
pub fn normalize_json(value: &Option<Value>) -> Option<Json<&Value>> {
    value.as_ref().map(Json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_json_handles_options() {
        let none: Option<Value> = None;
        assert!(normalize_json(&none).is_none());

        let some = Some(serde_json::json!({"score": 0.9}));
        let normalized = normalize_json(&some);
        assert!(normalized.is_some());
    }
}
