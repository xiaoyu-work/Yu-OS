use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use yuos_sdk::generated::{AiAppTool, AiChatRequest, AiToolTurn};

use crate::agent::llm::types::{ChatResponse, ContentBlock, FinishReason, Message, Role, Tool};
use crate::caps::manifest::AiSafety;
use crate::yuosd::wire::bounded::AI_CHAT_REQUEST_BYTES;

use super::gate::{apply_safety, AiError, ChatRequest, ProposedToolCall};

#[derive(Debug, Default, Clone)]
pub(crate) struct AppTools {
    tools: Vec<AiAppTool>,
    turns: Vec<AiToolTurn>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderState {
    provider: String,
    model: String,
    blocks: Vec<ContentBlock>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub(crate) struct ChatInput(AiChatRequest);

impl<'de> Deserialize<'de> for ChatInput {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let mut value = Value::deserialize(deserializer)?;
        if serde_json::to_vec(&value).map_err(D::Error::custom)?.len() > AI_CHAT_REQUEST_BYTES {
            return Err(D::Error::custom("App chat request exceeds its byte limit"));
        }
        yuos_sdk::generated::validate_ai_chat_request(&value).map_err(D::Error::custom)?;
        yuos_sdk::generated::normalize_ai_chat_request_integers(&mut value);
        serde_json::from_value(value)
            .map(Self)
            .map_err(D::Error::custom)
    }
}

impl ChatInput {
    pub(crate) fn into_request(self, app_id: String) -> ChatRequest {
        let request = self.0;
        ChatRequest {
            app_id,
            origin: request.origin,
            prompt: Some(request.prompt),
            system: request.system,
            max_units: request.max_units,
            images: request.images.unwrap_or_default(),
            app_tools: AppTools {
                tools: request.tools.unwrap_or_default(),
                turns: request.tool_turns.unwrap_or_default(),
            },
            ..Default::default()
        }
    }
}

impl AppTools {
    pub(super) fn project(
        &self,
        provider: &str,
        model: &str,
        safety: AiSafety,
        messages: &mut Vec<Message>,
        redacted: &mut bool,
    ) -> Result<Vec<Tool>, AiError> {
        if provider == "llama_local" && !self.tools.is_empty() {
            return Err(AiError::Provider(
                "the configured local text provider does not support tools".into(),
            ));
        }
        let mut names = HashSet::new();
        let mut tools = Vec::new();
        for tool in &self.tools {
            if !valid_name(&tool.name)
                || !names.insert(tool.name.as_str())
                || tool.parameters.get("type").and_then(Value::as_str) != Some("object")
            {
                return Err(AiError::InvalidToolTurn(
                    "tools need unique function names and object parameter schemas",
                ));
            }
            tools.push(Tool {
                name: tool.name.clone(),
                description: tool.description.clone(),
                input_schema: tool.parameters.clone(),
            });
        }

        for turn in &self.turns {
            let mut ids = HashSet::new();
            let mut results = HashMap::new();
            if turn
                .results
                .iter()
                .any(|result| results.insert(result.id.as_str(), result).is_some())
            {
                return Err(AiError::InvalidToolTurn(
                    "tool results must have unique call IDs",
                ));
            }
            let mut assistant = Vec::new();
            if let Some(text) = &turn.text {
                let (text, changed) = apply_safety(text, safety);
                *redacted |= changed;
                assistant.push(ContentBlock::Text { text });
            }
            if let Some(state) = &turn.provider_state {
                let state: ProviderState = serde_json::from_str(state)
                    .map_err(|_| AiError::InvalidToolTurn("invalid provider continuation state"))?;
                if state.provider != provider
                    || state.model != model
                    || !state.blocks.iter().all(|block| match block {
                        ContentBlock::Reasoning { .. } => true,
                        ContentBlock::ToolState { tool_use_id, .. } => {
                            turn.calls.iter().any(|call| call.id == *tool_use_id)
                        }
                        _ => false,
                    })
                {
                    return Err(AiError::InvalidToolTurn(
                        "provider state does not match the configured model and tool calls",
                    ));
                }
                assistant.extend(state.blocks);
            }
            let mut output = Vec::new();
            for call in &turn.calls {
                if !valid_id(&call.id)
                    || !names.contains(call.name.as_str())
                    || !ids.insert(call.id.as_str())
                {
                    return Err(AiError::InvalidToolTurn(
                        "tool calls must match declared functions and have unique IDs",
                    ));
                }
                let result = results
                    .remove(call.id.as_str())
                    .ok_or(AiError::InvalidToolTurn(
                        "provide one result for every tool call",
                    ))?;
                let (content, changed) = apply_safety(&result.content, safety);
                *redacted |= changed;
                assistant.push(ContentBlock::ToolUse {
                    id: call.id.clone(),
                    name: call.name.clone(),
                    input: call.input.clone(),
                });
                output.push(ContentBlock::ToolResult {
                    tool_use_id: call.id.clone(),
                    content,
                    is_error: result.is_error.unwrap_or(false),
                });
            }
            if !results.is_empty() {
                return Err(AiError::InvalidToolTurn(
                    "tool result has no matching call ID",
                ));
            }
            messages.push(Message {
                role: Role::Assistant,
                content: assistant,
            });
            messages.push(Message {
                role: Role::Tool,
                content: output,
            });
        }
        Ok(tools)
    }

    pub(super) fn proposals(
        &self,
        provider: &str,
        model: &str,
        response: ChatResponse,
    ) -> Result<(Vec<ProposedToolCall>, Option<String>), AiError> {
        let mut seen = HashSet::new();
        let calls = response.tool_calls;
        if calls.len() > 16
            || calls.iter().any(|call| {
                !valid_id(&call.id)
                    || !seen.insert(call.id.as_str())
                    || !call.input.is_object()
                    || !self.tools.iter().any(|tool| tool.name == call.name)
            })
            || response.content.iter().any(|block| match block {
                ContentBlock::ToolUse { id, name, input } => !calls
                    .iter()
                    .any(|call| call.id == *id && call.name == *name && call.input == *input),
                _ => false,
            })
            || (calls.is_empty() && response.finish_reason == FinishReason::ToolUse)
        {
            return Err(AiError::Provider(
                "text model returned an invalid or undeclared App tool call".into(),
            ));
        }
        if calls.is_empty() {
            return Ok((Vec::new(), None));
        }
        let blocks: Vec<_> = response
            .content
            .into_iter()
            .filter(|block| match block {
                ContentBlock::Reasoning { .. } => true,
                ContentBlock::ToolState { tool_use_id, .. } => seen.contains(tool_use_id.as_str()),
                _ => false,
            })
            .collect();
        let provider_state = if blocks.is_empty() {
            None
        } else {
            let state = serde_json::to_string(&ProviderState {
                provider: provider.into(),
                model: model.into(),
                blocks,
            })
            .map_err(|error| AiError::Internal(error.to_string()))?;
            if state.len() > 65_536 {
                return Err(AiError::Provider(
                    "text model continuation state exceeds its byte limit".into(),
                ));
            }
            Some(state)
        };
        Ok((
            calls
                .into_iter()
                .map(|call| ProposedToolCall {
                    id: call.id,
                    name: call.name,
                    input: call.input,
                })
                .collect(),
            provider_state,
        ))
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 256 && !id.chars().any(char::is_control)
}
