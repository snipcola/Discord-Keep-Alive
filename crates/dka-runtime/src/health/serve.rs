use std::future::Future;
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::watch;
use tracing::{debug, trace};

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
  std::str::from_utf8(buf)
    .unwrap_or("")
    .trim()
    .eq_ignore_ascii_case("ok")
}

async fn respond_status(w: &mut (impl AsyncWriteExt + Unpin), state: &HealthState) {
  let body = state.status_line();
  trace!(status = %body.trim(), "health probe response");
  if let Err(err) = w.write_all(body.as_bytes()).await {
    debug!(error = %err, "health write failed");
  }
}

async fn read_probe_status(r: &mut (impl AsyncReadExt + Unpin)) -> Result<bool> {
  let mut buf = [0u8; 64];
  let n = r.read(&mut buf).await.context("read health response")?;
  Ok(parse_status(&buf[..n]))
}

async fn serve_loop<F, Fut, S>(
  shutdown: &mut watch::Receiver<bool>,
  state: Arc<HealthState>,
  mut next: F,
) -> Result<()>
where
  F: FnMut() -> Fut,
  Fut: Future<Output = Result<Option<S>>>,
  S: AsyncWriteExt + Unpin,
{
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
      accepted = next() => {
        if let Some(mut stream) = accepted? {
          respond_status(&mut stream, &state).await;
        }
      }
    }
  }
  Ok(())
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

    serve_loop(shutdown, state, || async {
      match listener.accept().await {
        Ok((stream, _)) => Ok(Some(stream)),
        Err(err) => {
          debug!(error = %err, "health accept failed");
          Ok(None)
        }
      }
    })
    .await?;

    let _ = std::fs::remove_file(path);
    Ok(())
  }

  pub async fn probe_once(endpoint: &str) -> Result<bool> {
    use tokio::net::UnixStream;
    let mut stream = UnixStream::connect(endpoint)
      .await
      .with_context(|| format!("connect health socket {endpoint}"))?;
    read_probe_status(&mut stream).await
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

    serve_loop(shutdown, state, || {
      let name = name.as_str();
      async move {
        let server = ServerOptions::new()
          .create(name)
          .with_context(|| format!("create health pipe {name}"))?;
        match server.connect().await {
          Ok(()) => Ok(Some(server)),
          Err(err) => {
            debug!(error = %err, "health pipe connect failed");
            Ok(None)
          }
        }
      }
    })
    .await
  }

  pub async fn probe_once(endpoint: &str) -> Result<bool> {
    use tokio::net::windows::named_pipe::ClientOptions;
    let name = pipe_path(endpoint);
    let mut client = ClientOptions::new()
      .open(&name)
      .with_context(|| format!("open health pipe {name}"))?;
    read_probe_status(&mut client).await
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
