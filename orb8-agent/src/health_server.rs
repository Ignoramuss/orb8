use crate::health::HealthState;
use log::{error, info};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

pub async fn run(health: HealthState, port: u16, cancel: CancellationToken) {
    let addr = format!("0.0.0.0:{}", port);
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => {
            info!("Health server listening on {}", addr);
            l
        }
        Err(e) => {
            error!("Failed to bind health server on {}: {}", addr, e);
            return;
        }
    };

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("Health server shutting down");
                return;
            }
            accept = listener.accept() => {
                let (mut stream, _) = match accept {
                    Ok(conn) => conn,
                    Err(e) => {
                        error!("Health server accept error: {}", e);
                        continue;
                    }
                };

                let health = health.clone();
                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    let n = match stream.read(&mut buf).await {
                        Ok(n) if n > 0 => n,
                        _ => return,
                    };

                    let request = String::from_utf8_lossy(&buf[..n]);
                    let path = request
                        .lines()
                        .next()
                        .and_then(|line| line.split_whitespace().nth(1))
                        .unwrap_or("");

                    let (status, body) = match path {
                        "/healthz" => {
                            if health.is_healthy() {
                                ("200 OK", health.health_message())
                            } else {
                                ("503 Service Unavailable", health.health_message())
                            }
                        }
                        "/readyz" => {
                            if health.is_ready() {
                                ("200 OK", "ready".to_string())
                            } else {
                                ("503 Service Unavailable", "not ready: probes not attached".to_string())
                            }
                        }
                        _ => ("404 Not Found", "not found".to_string()),
                    };

                    let response = format!(
                        "HTTP/1.1 {}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        status,
                        body.len(),
                        body
                    );

                    let _ = stream.write_all(response.as_bytes()).await;
                });
            }
        }
    }
}
