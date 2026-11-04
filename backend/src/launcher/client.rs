//! Client for the launcher socket `NAS_LAUNCHER_SOCKET`, for background work and run operations.
//! Without the socket nothing is sent, and no launcher answer ever stops a backup.

use serde_json::{json, Value};
use std::{path::PathBuf, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
};

const ID: &str = "nas-backup-manager";
const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Default)]
pub struct Launcher {
    socket: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub status: u16,
    pub body: String,
}

impl Launcher {
    pub fn new(socket: Option<PathBuf>) -> Self {
        Self { socket }
    }

    pub fn from_env() -> Self {
        Self::new(
            std::env::var_os("NAS_LAUNCHER_SOCKET")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
        )
    }

    pub async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Answer, String> {
        let socket = self.socket.as_ref().ok_or("no launcher socket")?;
        let body = body.map(Value::to_string).unwrap_or_default();
        let exchange = async {
            let mut stream = UnixStream::connect(socket)
                .await
                .map_err(|error| format!("the launcher is not reachable: {error}"))?;
            let request = format!(
                "{method} {path} HTTP/1.1\r\nHost: nas\r\nConnection: close\r\n\
                 Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            stream
                .write_all(request.as_bytes())
                .await
                .map_err(|error| format!("the launcher did not take the request: {error}"))?;
            let mut raw = Vec::new();
            stream
                .read_to_end(&mut raw)
                .await
                .map_err(|error| format!("the launcher did not answer: {error}"))?;
            parse(&raw)
        };
        tokio::time::timeout(TIMEOUT, exchange)
            .await
            .map_err(|_| "the launcher did not answer in time".to_owned())?
    }

    /// Declares whether the app has work to do with its window closed.
    pub async fn background(&self, needed: bool) {
        if self.socket.is_none() {
            return;
        }
        match self
            .request(
                "PUT",
                "/api/apps/self/background",
                Some(&json!({ "needed": needed })),
            )
            .await
        {
            Ok(answer) if answer.status < 300 => {}
            Ok(answer) => eprintln!("<4>the launcher refused the background: {}", answer.body),
            Err(reason) => eprintln!("<4>the background was not declared: {reason}"),
        }
    }

    pub async fn notify(&self, title: &str, message: &str, tone: &str) {
        if self.socket.is_none() {
            return;
        }
        let notification = json!({
            "title": title,
            "message": message,
            "source": ID,
            "tone": tone,
        });
        if let Err(reason) = self
            .request("POST", "/api/notifications", Some(&notification))
            .await
        {
            eprintln!("<4>the notification was not sent: {reason}");
        }
    }

    /// Registers a run as a launcher operation that only a stop can interrupt.
    pub async fn start_task(&self, id: &str, name: &str) -> bool {
        let task = json!({
            "id": id,
            "source": ID,
            "name": name,
            "pausable": false,
            "cancellable": true,
        });
        self.socket.is_some()
            && self
                .request("POST", "/api/tasks", Some(&task))
                .await
                .is_ok_and(|answer| answer.status < 300)
    }

    /// Reports the progress of a run, or closes it with `state`.
    pub async fn update_task(&self, id: &str, update: &Value) {
        if self.socket.is_some() {
            let _ = self
                .request("PATCH", &format!("/api/tasks/{id}"), Some(update))
                .await;
        }
    }

    /// Tells whether the user stopped the operation from the launcher widget.
    pub async fn cancelled(&self, id: &str) -> bool {
        let Ok(answer) = self.request("GET", "/api/tasks", None).await else {
            return false;
        };
        serde_json::from_str::<Vec<Value>>(&answer.body).is_ok_and(|tasks| {
            tasks
                .iter()
                .any(|task| task["id"] == id && task["state"] == "cancelled")
        })
    }
}

/// Parses the status and body of a response, chunked or not.
pub fn parse(raw: &[u8]) -> Result<Answer, String> {
    let end = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or("the launcher answered something that is not HTTP")?;
    let head = String::from_utf8_lossy(&raw[..end]);
    let body = &raw[end + 4..];
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .ok_or("the launcher answered without a status")?;
    let chunked = head.lines().any(|line| {
        line.to_ascii_lowercase()
            .starts_with("transfer-encoding: chunked")
    });
    let body = if chunked {
        dechunk(body)
    } else {
        body.to_vec()
    };
    Ok(Answer {
        status,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

fn dechunk(mut body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(line) = body.windows(2).position(|window| window == b"\r\n") {
        let size = std::str::from_utf8(&body[..line])
            .ok()
            .and_then(|size| usize::from_str_radix(size.trim(), 16).ok());
        let rest = &body[line + 2..];
        match size {
            Some(size) if size > 0 && rest.len() >= size => {
                out.extend_from_slice(&rest[..size]);
                body = rest[size..].strip_prefix(b"\r\n").unwrap_or(&rest[size..]);
            }
            _ => break,
        }
    }
    out
}

#[cfg(test)]
#[path = "../../tests/unit/launcher/client.rs"]
mod tests;
