use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::watch;
use tracing::debug;

use super::HealthState;

pub async fn serve(
  endpoint: &str,
  state: Arc<HealthState>,
  mut shutdown: watch::Receiver<bool>,
) -> Result<()> {
  platform::serve(endpoint, state, &mut shutdown).await
}

pub async fn probe_once(endpoint: &str) -> Result<bool> {
  platform::probe_once(endpoint).await
}

fn parse_status(buf: &[u8]) -> bool {
  let text = std::str::from_utf8(buf).unwrap_or("").trim();
  text.eq_ignore_ascii_case("ok")
}

#[cfg(unix)]
mod platform {
  use std::path::Path;

  use super::*;

  pub async fn serve(
    endpoint: &str,
    state: Arc<HealthState>,
    shutdown: &mut watch::Receiver<bool>,
  ) -> Result<()> {
    use tokio::net::UnixListener;

    let path = Path::new(endpoint);
    if let Some(parent) = path.parent()
      && !parent.as_os_str().is_empty()
    {
      std::fs::create_dir_all(parent)
        .with_context(|| format!("create health socket directory {}", parent.display()))?;
    }
    if path.exists() {
      let _ = std::fs::remove_file(path);
    }

    let listener =
      UnixListener::bind(path).with_context(|| format!("bind health socket {}", path.display()))?;
    debug!(path = %path.display(), "health socket listening");

    loop {
      if *shutdown.borrow() {
        break;
      }

      tokio::select! {
        changed = shutdown.changed() => {
          if changed.is_err() || *shutdown.borrow() {
            break;
          }
        }
        accepted = listener.accept() => {
          match accepted {
            Ok((mut stream, _)) => {
              let body = state.status_line();
              if let Err(err) = stream.write_all(body.as_bytes()).await {
                debug!(error = %err, "health write failed");
              }
            }
            Err(err) => {
              debug!(error = %err, "health accept failed");
            }
          }
        }
      }
    }

    let _ = std::fs::remove_file(path);
    Ok(())
  }

  pub async fn probe_once(endpoint: &str) -> Result<bool> {
    use tokio::net::UnixStream;

    let mut stream = UnixStream::connect(endpoint)
      .await
      .with_context(|| format!("connect health socket {endpoint}"))?;
    let mut buf = [0u8; 64];
    let n = stream
      .read(&mut buf)
      .await
      .context("read health socket response")?;
    Ok(parse_status(&buf[..n]))
  }
}

#[cfg(windows)]
mod platform {
  use super::*;

  fn pipe_path(endpoint: &str) -> String {
    let trimmed = endpoint.trim();
    if trimmed.starts_with(r"\\.\pipe\") || trimmed.starts_with("//./pipe/") {
      trimmed.to_string()
    } else {
      format!(r"\\.\pipe\{trimmed}")
    }
  }

  pub async fn serve(
    endpoint: &str,
    state: Arc<HealthState>,
    shutdown: &mut watch::Receiver<bool>,
  ) -> Result<()> {
    use tokio::net::windows::named_pipe::ServerOptions;

    let name = pipe_path(endpoint);
    debug!(pipe = %name, "health pipe listening");

    loop {
      if *shutdown.borrow() {
        break;
      }

      let mut server = ServerOptions::new()
        .create(&name)
        .with_context(|| format!("create health pipe {name}"))?;

      tokio::select! {
        changed = shutdown.changed() => {
          if changed.is_err() || *shutdown.borrow() {
            break;
          }
        }
        connected = server.connect() => {
          if let Err(err) = connected {
            debug!(error = %err, "health pipe connect failed");
            continue;
          }
          let body = state.status_line();
          if let Err(err) = server.write_all(body.as_bytes()).await {
            debug!(error = %err, "health write failed");
          }
        }
      }
    }

    Ok(())
  }

  pub async fn probe_once(endpoint: &str) -> Result<bool> {
    use tokio::net::windows::named_pipe::ClientOptions;

    let name = pipe_path(endpoint);
    let mut client = ClientOptions::new()
      .open(&name)
      .with_context(|| format!("open health pipe {name}"))?;
    let mut buf = [0u8; 64];
    let n = client
      .read(&mut buf)
      .await
      .context("read health pipe response")?;
    Ok(parse_status(&buf[..n]))
  }
}

#[cfg(not(any(unix, windows)))]
mod platform {
  use super::*;

  pub async fn serve(
    _endpoint: &str,
    _state: Arc<HealthState>,
    _shutdown: &mut watch::Receiver<bool>,
  ) -> Result<()> {
    anyhow::bail!("health socket is not supported on this platform")
  }

  pub async fn probe_once(_endpoint: &str) -> Result<bool> {
    anyhow::bail!("health socket is not supported on this platform")
  }
}
