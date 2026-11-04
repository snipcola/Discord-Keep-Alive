use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::watch;
use tokio::time::sleep;
use tracing::{debug, trace};

use super::HealthState;

const ACCEPT_RETRY_DELAY: Duration = Duration::from_millis(100);

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
        match accepted? {
          Some(mut stream) => respond_status(&mut stream, &state).await,
          None => sleep(ACCEPT_RETRY_DELAY).await,
        }
      }
    }
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use tokio::io::{DuplexStream, duplex};

  use super::*;

  async fn request_status(live: bool) -> String {
    let (mut client, server) = duplex(64);
    let (tx, mut rx) = watch::channel(false);
    let state = HealthState::new(["a"]);
    state.set_live("a", live);

    let mut pending_stream = Some(server);
    let serving = serve_loop(&mut rx, state, || {
      let stream = pending_stream.take();
      async move {
        match stream {
          Some(stream) => Ok(Some(stream)),
          None => std::future::pending().await,
        }
      }
    });
    let probing = async {
      let mut body = String::new();
      client.read_to_string(&mut body).await.unwrap();
      tx.send(true).unwrap();
      body
    };
    let (served, body) = tokio::join!(serving, probing);
    served.unwrap();
    body
  }

  #[tokio::test]
  async fn serve_loop_answers_with_status_line() {
    assert_eq!(
      request_status(true).await,
      "ok\n",
      "status line for live=true"
    );
    assert_eq!(
      request_status(false).await,
      "fail\n",
      "status line for live=false"
    );
  }

  #[tokio::test]
  async fn serve_loop_returns_immediately_when_shutdown_is_set() {
    let (_tx, mut rx) = watch::channel(true);
    let state = HealthState::new(["a"]);
    let mut calls = 0u32;
    serve_loop(&mut rx, state, || {
      calls += 1;
      async { Ok(None::<DuplexStream>) }
    })
    .await
    .unwrap();
    assert_eq!(calls, 0, "accept calls when shutdown is pre-set");
  }

  #[tokio::test]
  async fn serve_loop_propagates_accept_errors() {
    let (_tx, mut rx) = watch::channel(false);
    let state = HealthState::new(["a"]);
    let err = serve_loop(&mut rx, state, || async {
      Err::<Option<DuplexStream>, _>(anyhow::anyhow!("pipe create failed"))
    })
    .await
    .unwrap_err();
    assert_eq!(
      err.to_string(),
      "pipe create failed",
      "accept error text must propagate unchanged"
    );
  }

  #[tokio::test(start_paused = true)]
  async fn serve_loop_backs_off_after_failed_accept() {
    let (tx, mut rx) = watch::channel(false);
    let state = HealthState::new(["a"]);
    let mut calls = 0u32;
    let serving = serve_loop(&mut rx, state, || {
      calls += 1;
      async { Ok(None::<DuplexStream>) }
    });
    let stopping = async {
      sleep(ACCEPT_RETRY_DELAY * 3 + ACCEPT_RETRY_DELAY / 2).await;
      tx.send(true).unwrap();
    };
    let (served, ()) = tokio::join!(serving, stopping);
    served.unwrap();
    assert_eq!(calls, 4, "accept calls within 350 ms of paused time");
  }
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

  #[cfg(test)]
  mod tests {
    use super::pipe_path;

    #[test]
    fn pipe_path_normalises_endpoint() {
      let cases = [
        ("dka-health", r"\\.\pipe\dka-health"),
        ("  dka-health\n", r"\\.\pipe\dka-health"),
        (r"\\.\pipe\dka-health", r"\\.\pipe\dka-health"),
        ("//./pipe/dka-health", "//./pipe/dka-health"),
        (r"\\.\pipe\a\b", r"\\.\pipe\a\b"),
      ];
      for (endpoint, expected) in cases {
        assert_eq!(pipe_path(endpoint), expected, "{endpoint:?}");
      }
    }
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
