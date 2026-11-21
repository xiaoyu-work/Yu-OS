use async_trait::async_trait;
use serde_json::{json, Value};

use crate::model::tasks::decision::{DecisionModel, DecisionRequest};

use super::{exposure::ToolExposure, Tool, ToolResult};

pub(super) struct Decide(pub DecisionModel);

#[async_trait]
impl Tool for Decide {
    fn name(&self) -> &str {
        "yu_decide"
    }

    fn description(&self) -> &str {
        "Ask the configured local decision model up to eight bounded questions about one state. \
         Use for classification, selection or ranking, not for generating prose or deciding authority."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "state": {
                    "type": ["object", "array", "string"],
                    "description": "Only the evidence needed for the questions."
                },
                "questions": {
                    "type": "object",
                    "description": "Question IDs mapped to objects with type and instructions. \
                        choice needs criteria as an object mapping option IDs to labels; \
                        score needs criteria as an ordered array of labels; noul takes no criteria.",
                    "minProperties": 1,
                    "maxProperties": 8,
                    "additionalProperties": {"type": "object"}
                }
            },
            "required": ["state", "questions"],
            "additionalProperties": false
        })
    }

    fn exposure(&self) -> ToolExposure {
        ToolExposure::always().requiring_app_service_caller()
    }

    async fn exec(&self, input: Value) -> ToolResult {
        let Ok(request) = serde_json::from_value::<DecisionRequest>(input) else {
            return ToolResult::err("invalid decision request");
        };
        match self.0.decide(&request).await {
            Ok(result) => match serde_json::to_string(&result) {
                Ok(result) => ToolResult::ok(result),
                Err(_) => ToolResult::err("could not encode the decision result"),
            },
            Err(error) => ToolResult::err(error),
        }
    }
}
