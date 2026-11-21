use super::*;
use std::os::unix::fs::PermissionsExt;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

#[tokio::test]
async fn private_socket_batches_and_validates_decisions() {
    let request: DecisionRequest = serde_json::from_value(json!({
        "state": {"label": "Calendar"},
        "questions": {
            "route": {"type": "choice", "instructions": "Choose the app.", "criteria": {
                "calendar": "Calendar", "mail": "Mail"
            }},
            "visible": {"type": "noul", "instructions": "Is there a label?"},
            "priority": {"type": "score", "instructions": "Rate priority.", "criteria": ["Low", "High"]}
        }
    }))
    .unwrap();
    let mut reply = json!({
        "model": "fixture",
        "answers": {
            "route": {"type": "choice", "choice": "calendar", "probabilities": {
                "calendar": 0.99, "mail": 0.01
            }},
            "visible": {"type": "noul", "noul": 0.97},
            "priority": {"type": "score", "score": 0.8, "probabilities": {"0": 0.2, "1": 0.8}}
        }
    });
    let directory = tempfile::tempdir().unwrap();
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let socket = directory.path().join("decision.sock");
    let listener = tokio::net::UnixListener::bind(&socket).unwrap();
    let owner = std::fs::metadata(directory.path()).unwrap().uid();
    check_socket(&socket, owner).unwrap();
    let model = DecisionModel::new("fixture", socket, owner, Duration::from_secs(2)).unwrap();
    for accepted in [true, false] {
        if !accepted {
            reply["answers"]["route"]["choice"] = json!("not_offered");
        }
        let payload = reply.to_string();
        let server = async {
            let (stream, _) = listener.accept().await.unwrap();
            let mut stream = BufReader::new(stream);
            let mut line = String::new();
            stream.read_line(&mut line).await.unwrap();
            assert!(line.starts_with("POST /v1/systemone "));
            let mut length = None;
            loop {
                line.clear();
                stream.read_line(&mut line).await.unwrap();
                if line.trim().is_empty() {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = Some(value.trim().parse::<usize>().unwrap());
                }
            }
            let mut body = vec![0; length.unwrap()];
            stream.read_exact(&mut body).await.unwrap();
            let wire: Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(wire["questions"].as_object().unwrap().len(), 3);
            assert_eq!(wire["state"]["label"], "Calendar");
            assert_eq!(wire["model"], "fixture");
            stream
                .get_mut()
                .write_all(
                    format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                    payload.len()
                )
                    .as_bytes(),
                )
                .await
                .unwrap();
        };
        let (result, ()) = tokio::time::timeout(Duration::from_secs(5), async {
            tokio::join!(model.decide(&request), server)
        })
        .await
        .unwrap();
        if accepted {
            assert!(matches!(
                result.unwrap().answers["route"],
                Answer::Choice { ref choice, .. } if choice == "calendar"
            ));
        } else {
            assert!(result.unwrap_err().contains("does not match"));
        }
    }
}
