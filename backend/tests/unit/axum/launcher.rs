use super::*;
use crate::test_support::Scratch;
use std::sync::{Arc, Mutex};
use tokio::net::UnixListener;

#[test]
fn reads_plain_and_chunked_answers() {
    let plain = parse(b"HTTP/1.1 204 No Content\r\ncontent-length: 0\r\n\r\n").expect("plain");
    assert_eq!(
        plain,
        Answer {
            status: 204,
            body: String::new()
        }
    );
    let chunked = parse(
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\n[{\"a\r\n8\r\n\":\"è\"}]\r\n0\r\n\r\n"
            .as_bytes(),
    )
    .expect("chunked");
    assert_eq!(chunked.body, "[{\"a\":\"è\"}]");
    assert!(parse(b"not http").is_err());
    assert!(parse(b"HTTP/1.1 nope\r\n\r\n").is_err());
}

/// A launcher on a socket of the test: it writes down each request and answers with `body`.
fn launcher(socket: &std::path::Path, body: &'static str) -> Arc<Mutex<Vec<String>>> {
    let listener = UnixListener::bind(socket).expect("socket");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let kept = Arc::clone(&seen);
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            // The whole request: its head, then as much body as it announces.
            let mut request = Vec::new();
            let mut buffer = [0; 4096];
            loop {
                let read = stream.read(&mut buffer).await.unwrap_or(0);
                request.extend_from_slice(&buffer[..read]);
                let text = String::from_utf8_lossy(&request).into_owned();
                let complete = text.split_once("\r\n\r\n").is_some_and(|(head, body)| {
                    let length = head
                        .lines()
                        .find_map(|line| line.strip_prefix("Content-Length: "))
                        .and_then(|length| length.parse::<usize>().ok())
                        .unwrap_or(0);
                    body.len() >= length
                });
                if read == 0 || complete {
                    break;
                }
            }
            kept.lock()
                .expect("requests")
                .push(String::from_utf8_lossy(&request).into_owned());
            let answer = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(answer.as_bytes()).await;
        }
    });
    seen
}

#[tokio::test]
async fn says_what_it_does_on_the_socket_of_the_launcher() {
    let folder = Scratch::new("launcher");
    let socket = folder.path().join("apps.sock");
    let seen = launcher(
        &socket,
        r#"[{"id":"run-1","state":"cancelled"},{"id":"run-2","state":"running"}]"#,
    );
    let door = Launcher::new(Some(socket));

    door.background(true).await;
    assert!(door.start_task("run-1", "Backup: Photos").await);
    door.update_task("run-1", &json!({ "detail": "3 file" }))
        .await;
    assert!(door.cancelled("run-1").await);
    assert!(!door.cancelled("run-2").await);

    let seen = seen.lock().expect("requests").clone();
    assert!(seen[0].starts_with("PUT /api/apps/self/background HTTP/1.1"));
    assert!(seen[0].ends_with(r#"{"needed":true}"#));
    assert!(seen[1].starts_with("POST /api/tasks "));
    assert!(seen[1].contains(r#""source":"nas-backup-manager""#));
    assert!(seen[1].contains(r#""cancellable":true"#));
    assert!(seen[2].starts_with("PATCH /api/tasks/run-1 "));
}

#[tokio::test]
async fn does_nothing_without_a_launcher() {
    let none = Launcher::new(None);
    none.background(false).await;
    assert!(!none.start_task("run", "Backup").await);
    none.update_task("run", &json!({})).await;
    assert!(!none.cancelled("run").await);
    assert!(none.request("GET", "/", None).await.is_err());

    let gone = Launcher::new(Some(std::env::temp_dir().join("nas-backup-no-socket")));
    assert!(gone.request("GET", "/api/tasks", None).await.is_err());
    gone.background(true).await;
    assert!(Launcher::from_env()
        .request("GET", "/", None)
        .await
        .is_err());
}
