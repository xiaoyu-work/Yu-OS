use std::{future::Future, time::Duration};

use serde_json::{json, Value};
use tokio::sync::mpsc;

use super::{
    protocol::{encode_response, Response},
    routes::Command,
    transport::frame::PeerStream,
    wire::RequestId,
};

pub(crate) const MAX_DELTA_BYTES: usize = 16 * 1024;

tokio::task_local! {
    pub(crate) static OUTPUT: mpsc::Sender<String>;
}

pub(crate) fn requested(command: Command, params: &Value) -> bool {
    match command {
        Command::AiChat => params.get("stream").and_then(Value::as_bool) == Some(true),
        Command::AppSessionRelay
            if params.get("command").and_then(Value::as_str) == Some(Command::AiChat.as_str()) =>
        {
            requested(Command::AiChat, &params["params"])
        }
        _ => false,
    }
}

pub(crate) fn delta(id: RequestId, text: &str) -> Response {
    Response::ok(id, json!({ "chat_delta": text }))
}

pub(crate) async fn dispatch(
    stream: &mut PeerStream,
    id: RequestId,
    deadline: Duration,
    run: impl Future<Output = Response>,
) -> Response {
    let (tx, mut rx) = mpsc::channel(1);
    let run = OUTPUT.scope(tx, run);
    tokio::pin!(run);
    // Poll the provider as well as the socket, but hold at most one buffered delta.
    loop {
        let text = tokio::select! {
            response = &mut run => {
                if let Ok(text) = rx.try_recv() {
                    let _ = write_delta(stream, id, deadline, &text).await;
                }
                return response;
            }
            _ = stream.wait_for_input() => {
                rx.close();
                return run.await;
            }
            Some(text) = rx.recv() => text,
        };
        if !write_delta(stream, id.clone(), deadline, &text).await {
            rx.close();
            return run.await;
        }
    }
}

async fn write_delta(
    stream: &mut PeerStream,
    id: RequestId,
    deadline: Duration,
    text: &str,
) -> bool {
    let Ok(body) = encode_response(&delta(id, text)) else {
        return false;
    };
    matches!(
        tokio::time::timeout(deadline, stream.write_response(&body)).await,
        Ok(Ok(()))
    )
}
