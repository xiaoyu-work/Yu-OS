//! Setup for the owner-local decision modality.

use std::collections::BTreeMap;
use std::io::{IsTerminal, Write};
use std::path::Path;

use serde_json::{json, Value};

use super::{config_path, read_config_or_empty, read_line, write_config_atomic, ApplyArgs};
use crate::config::DecisionConfig;
use crate::model::tasks::decision::{
    self as model, DecisionModel, DecisionRequest, Question, LOCAL_PROVIDER,
};

pub(super) fn providers() -> Value {
    json!({
        "modality": "decision",
        "default_provider": LOCAL_PROVIDER,
        "providers": [{
            "name": LOCAL_PROVIDER,
            "label": "Local System One",
            "needs_credential": false,
            "models": [{"name": "h2oai/h2o-lightning-4b"}],
            "default_model": "h2oai/h2o-lightning-4b",
        }],
    })
}

pub(super) fn status() -> Value {
    let cfg = crate::config::current_snapshot();
    let mut status = model::status(&cfg.decision);
    let ready = status["socket_present"] == true;
    status["modality"] = json!("decision");
    status["ready"] = json!(ready);
    status["config_path"] = json!(config_path().display().to_string());
    if cfg.decision.model.is_empty() {
        status["provider"] = json!("none");
    } else if !ready {
        status["reason"] = json!({"error": "Local decision service is unavailable"});
    }
    status
}

pub(super) fn verify() -> Value {
    verify_config(&crate::config::load_user_config().decision)
}

fn verify_config(cfg: &DecisionConfig) -> Value {
    let mut result = json!({
        "modality": "decision",
        "kind": "local inference",
        "provider": LOCAL_PROVIDER,
        "model": cfg.model,
        "attempted": false,
        "ok": false,
    });
    let probe = (|| {
        let model = DecisionModel::from_config(cfg)?.ok_or("No decision model is configured")?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "Could not start the decision test")?;
        let request = DecisionRequest {
            state: json!("The desk is empty."),
            questions: BTreeMap::from([(
                "empty".into(),
                Question::Noul {
                    instructions: "Does the statement say the desk is empty?".into(),
                },
            )]),
        };
        result["attempted"] = json!(true);
        runtime.block_on(model.decide(&request)).map(|_| ())
    })();
    match probe {
        Ok(()) => result["ok"] = json!(true),
        Err(error) => result["reason"] = json!({"error": error}),
    }
    result
}

pub(super) fn apply(args: ApplyArgs) -> Result<Value, String> {
    if args.api_key.is_some()
        || args.api_key_stdin
        || args.api_key_env.is_some()
        || args.base_url.is_some()
        || args.api_version.is_some()
    {
        return Err("the local decision model accepts no credentials or network endpoint".into());
    }
    match args.provider.as_deref().map(str::trim) {
        Some(LOCAL_PROVIDER) => {}
        Some("none") => return reset(),
        _ => {
            return Err(format!(
                "decision setup requires --provider {LOCAL_PROVIDER} (or none)"
            ))
        }
    }
    let alias = args
        .model
        .as_deref()
        .ok_or("--model is required for decision setup")?;
    set_model(&config_path(), Some(alias.trim()))
}

pub(super) fn reset() -> Result<Value, String> {
    set_model(&config_path(), None)
}

fn set_model(path: &Path, alias: Option<&str>) -> Result<Value, String> {
    if let Some(alias) = alias {
        model::validate_model_alias(alias)?;
    }
    let mut config = read_config_or_empty(path)?;
    let root = config
        .as_object_mut()
        .ok_or("model settings must be a JSON object")?;
    if let Some(alias) = alias {
        let block = root.entry("decision").or_insert_with(|| json!({}));
        let block = block
            .as_object_mut()
            .ok_or("decision settings must be a JSON object")?;
        block.insert("model".into(), json!(alias));
    } else {
        root.remove("decision");
    }
    write_config_atomic(path, &config)?;
    Ok(json!({
        "ok": true,
        "modality": "decision",
        "provider": if alias.is_some() { LOCAL_PROVIDER } else { "none" },
        "model": alias.unwrap_or_default(),
        "key_source": "not-required",
        "config_path": path.display().to_string(),
    }))
}

pub(super) fn wizard(verify_after: bool) -> Result<Value, String> {
    if !std::io::stdin().is_terminal() {
        return Err(format!(
            "decision setup requires a terminal; use `yu agent setup decision apply --provider {LOCAL_PROVIDER} --model ALIAS`"
        ));
    }
    let cfg = crate::config::load_user_config();
    let alias = {
        let mut output = std::io::stderr().lock();
        writeln!(output, "Local decision model (System One)").map_err(|error| error.to_string())?;
        writeln!(output, "Start a compatible owner-local service on /run/user/<uid>/yu-decision.sock. Setup does not download or start it.")
            .map_err(|error| error.to_string())?;
        write!(output, "Model alias [{}]: ", cfg.decision.model)
            .and_then(|()| output.flush())
            .map_err(|error| error.to_string())?;
        let input = read_line()?;
        if input.trim().is_empty() {
            cfg.decision.model.clone()
        } else {
            input.trim().to_owned()
        }
    };
    let mut result = set_model(&config_path(), Some(&alias))?;
    if verify_after {
        result["verification"] = verify();
    }
    Ok(result)
}
