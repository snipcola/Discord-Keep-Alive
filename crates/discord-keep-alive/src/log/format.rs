use std::fmt;

use time::format_description::StaticFormatDescription;
use time::macros::format_description;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::{FormatTime, LocalTime};
use tracing_subscriber::fmt::{FmtContext, FormatEvent};
use tracing_subscriber::registry::LookupSpan;

use super::account::AccountName;
use super::util::EventFields;
use crate::term::{BOLD, DIM, paint};

const TIME_FORMAT: StaticFormatDescription =
  format_description!("[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]");

pub(super) struct HumanFormat {
  ansi: bool,
  timer: LocalTime<StaticFormatDescription>,
}

impl HumanFormat {
  pub(super) fn new(ansi: bool) -> Self {
    Self {
      ansi,
      timer: LocalTime::new(TIME_FORMAT),
    }
  }
}

impl<S, N> FormatEvent<S, N> for HumanFormat
where
  S: Subscriber + for<'a> LookupSpan<'a>,
  N: for<'a> tracing_subscriber::fmt::FormatFields<'a> + 'static,
{
  fn format_event(
    &self,
    ctx: &FmtContext<'_, S, N>,
    mut writer: Writer<'_>,
    event: &Event<'_>,
  ) -> fmt::Result {
    let mut time = String::new();
    self.timer.format_time(&mut Writer::new(&mut time))?;
    write!(writer, "{} ", paint(self.ansi, DIM, time))?;
    write!(
      writer,
      "{} ",
      level_label(*event.metadata().level(), self.ansi)
    )?;

    let mut fields = EventFields::default();
    event.record(&mut fields);

    let account = fields.account.take().or_else(|| account_from_spans(ctx));
    if let Some(account) = account {
      write!(writer, "{}: ", paint(self.ansi, BOLD, account))?;
    }

    write!(writer, "{}", fields.message)?;
    for (key, value) in &fields.fields {
      write!(writer, " {key}=")?;
      write_value(&mut writer, value)?;
    }
    writeln!(writer)
  }
}

fn level_label(level: Level, ansi: bool) -> String {
  let (style, label) = match level {
    Level::ERROR => ("31", "ERROR"),
    Level::WARN => ("33", " WARN"),
    Level::INFO => ("32", " INFO"),
    Level::DEBUG => ("34", "DEBUG"),
    Level::TRACE => ("35", "TRACE"),
  };
  paint(ansi, style, label)
}

fn write_value(writer: &mut Writer<'_>, value: &str) -> fmt::Result {
  let needs_quotes = value.is_empty()
    || value
      .chars()
      .any(|c| c.is_whitespace() || c == '"' || c == '=');
  if needs_quotes {
    write!(writer, "{value:?}")
  } else {
    write!(writer, "{value}")
  }
}

fn account_from_spans<S, N>(ctx: &FmtContext<'_, S, N>) -> Option<String>
where
  S: Subscriber + for<'a> LookupSpan<'a>,
  N: for<'a> tracing_subscriber::fmt::FormatFields<'a> + 'static,
{
  let scope = ctx.event_scope()?;
  for span in scope {
    if let Some(AccountName(name)) = span.extensions().get::<AccountName>() {
      return Some(name.clone());
    }
  }
  None
}

#[cfg(test)]
mod tests {
  use std::io;
  use std::sync::{Arc, Mutex};

  use tracing_subscriber::fmt::MakeWriter;
  use tracing_subscriber::layer::SubscriberExt;

  use super::*;
  use crate::log::account::AccountLayer;

  #[derive(Clone, Default)]
  struct Buffer(Arc<Mutex<Vec<u8>>>);

  impl io::Write for Buffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
      self.0.lock().unwrap().extend_from_slice(buf);
      Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
      Ok(())
    }
  }

  impl MakeWriter<'_> for Buffer {
    type Writer = Buffer;

    fn make_writer(&self) -> Buffer {
      self.clone()
    }
  }

  fn render(emit: impl FnOnce()) -> String {
    let buffer = Buffer::default();
    let subscriber = tracing_subscriber::registry().with(AccountLayer).with(
      tracing_subscriber::fmt::layer()
        .event_format(HumanFormat::new(false))
        .with_writer(buffer.clone()),
    );
    tracing::subscriber::with_default(subscriber, emit);
    String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap()
  }

  #[test]
  fn account_prefixes_the_message_and_spaced_values_are_quoted() {
    let out = render(|| {
      tracing::error!(
        account = "main",
        reason = "Authentication failed.",
        code = 4004,
        "stopped"
      );
    });
    assert!(
      out.ends_with("ERROR main: stopped reason=\"Authentication failed.\" code=4004\n"),
      "{out}"
    );
  }
}
