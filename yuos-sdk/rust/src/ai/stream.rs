use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Child, Command, ExitStatus, Output, Stdio},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};

use serde_json::Value;

use super::{
    chat_wire_reply, dev_ai_url, dev_chat, encode_chat_request, structured_chat_request, AiError,
    AiResponse, ChatOpts,
};

const MAX_LINE_BYTES: usize = 8 * 1024 * 1024;
const MAX_TEXT_BYTES: usize = 512 * 1024;

/// Stream text from the configured model and return its final response.
pub fn chat_stream(
    prompt: &str,
    opts: ChatOpts,
    mut on_chunk: impl FnMut(&str),
) -> Result<AiResponse, AiError> {
    if prompt.trim().is_empty() {
        return Err(AiError::InvalidArg("chat: prompt must be non-empty".into()));
    }
    if opts.tools.as_ref().is_some_and(|tools| !tools.is_empty()) {
        return Err(AiError::InvalidArg(
            "chat: supply App functions in functions".into(),
        ));
    }
    let request = structured_chat_request(prompt, opts)?;
    let payload = encode_chat_request(&request)?;
    if dev_ai_url().is_some() {
        let reply = dev_chat(&request)?;
        if !reply.output.text.is_empty() {
            on_chunk(&reply.output.text);
        }
        return Ok(reply);
    }

    let binary = std::env::var_os("YUOS_KERNEL_BIN").unwrap_or_else(|| "yu".into());
    let mut child = Command::new(binary)
        .args(["--wire=1", "ai", "chat", "--request-stdin", "--stream"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(unavailable)?;
    let mut stdin = child.stdin.take().expect("piped chat stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("piped chat stdout"));
    let process = StreamChild::new(child);
    if let Err(error) = stdin.write_all(&payload) {
        if error.kind() != std::io::ErrorKind::BrokenPipe {
            return Err(unavailable(error));
        }
    }
    drop(stdin);

    let mut total = 0;
    let mut final_line = None;
    let mut frame = Vec::new();
    loop {
        frame.clear();
        if (&mut stdout)
            .take((MAX_LINE_BYTES + 1) as u64)
            .read_until(b'\n', &mut frame)
            .map_err(unavailable)?
            == 0
        {
            break;
        }
        if final_line.is_some() || frame.len() > MAX_LINE_BYTES || !frame.ends_with(b"\n") {
            return Err(unavailable("invalid App chat stream framing"));
        }
        let record: Value = serde_json::from_slice(&frame).map_err(unavailable)?;
        if record.get("event").is_some() {
            crate::generated::validate_ai_chat_delta(&record).map_err(unavailable)?;
            let text = record["text"].as_str().expect("validated chat delta text");
            total += text.len();
            if total > MAX_TEXT_BYTES {
                return Err(unavailable("App chat stream exceeded its text limit"));
            }
            on_chunk(text);
        } else {
            crate::generated::validate_envelope(&record).map_err(unavailable)?;
            final_line = Some(std::mem::take(&mut frame));
        }
    }
    let stdout =
        final_line.ok_or_else(|| unavailable("App chat stream ended without a final response"))?;
    let status = process.wait()?;
    chat_wire_reply(crate::decode_wire_response(
        "ai",
        "chat",
        Output {
            status,
            stdout,
            stderr: Vec::new(),
        },
    ))
}

fn unavailable(error: impl std::fmt::Display) -> AiError {
    AiError::Unavailable(error.to_string())
}

// The private watchdog can stop a stalled pipe without moving the caller's
// synchronous callback onto another thread.
struct StreamChild {
    child: Arc<Mutex<Child>>,
    stop: mpsc::Sender<()>,
    timer: Option<thread::JoinHandle<()>>,
}

impl StreamChild {
    fn new(child: Child) -> Self {
        let child = Arc::new(Mutex::new(child));
        let (stop, deadline) = mpsc::channel();
        let watched = Arc::clone(&child);
        let timer = thread::spawn(move || {
            if deadline.recv_timeout(Duration::from_secs(60))
                == Err(mpsc::RecvTimeoutError::Timeout)
            {
                let _ = watched.lock().expect("chat child lock").kill();
            }
        });
        Self {
            child,
            stop,
            timer: Some(timer),
        }
    }

    fn wait(&self) -> Result<ExitStatus, AiError> {
        loop {
            let status = self
                .child
                .lock()
                .expect("chat child lock")
                .try_wait()
                .map_err(unavailable)?;
            if let Some(status) = status {
                return Ok(status);
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}

impl Drop for StreamChild {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(timer) = self.timer.take() {
            let _ = timer.join();
        }
        let mut child = self.child.lock().expect("chat child lock");
        if child.try_wait().ok().flatten().is_none() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
