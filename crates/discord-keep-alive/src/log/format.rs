use std::fmt;

use time::format_description::StaticFormatDescription;
use time::macros::format_description;
use tracing::{Event, Subscriber};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::{FormatTime, LocalTime};
use tracing_subscriber::fmt::{FmtContext, FormatEvent};
use tracing_subscriber::registry::LookupSpan;

use super::account::AccountName;
use super::util::EventFields;

const TIME_FORMAT: StaticFormatDescription =
  format_description!("[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]");

pub(super) struct HumanFormat {
  pub ansi: bool,
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
    self.timer.format_time(&mut writer)?;
    write!(writer, " ")?;
    write_level(&mut writer, *event.metadata().level(), self.ansi)?;
    write!(writer, " ")?;

    let mut fields = EventFields::default();
    event.record(&mut fields);

    let account = fields.account.take().or_else(|| account_from_spans(ctx));
    if let Some(account) = account.as_deref() {
      write!(writer, "{account}: ")?;
    }

    write!(writer, "{}", fields.message)?;
    for (key, value) in &fields.fields {
      write!(writer, " {key}={value}")?;
    }
    writeln!(writer)
  }
}

fn write_level(writer: &mut Writer<'_>, level: tracing::Level, ansi: bool) -> fmt::Result {
  if !ansi {
    return write!(writer, "{level:>5}");
  }

  let (color, label) = match level {
    tracing::Level::ERROR => ("\x1b[31m", "ERROR"),
    tracing::Level::WARN => ("\x1b[33m", " WARN"),
    tracing::Level::INFO => ("\x1b[32m", " INFO"),
    tracing::Level::DEBUG => ("\x1b[34m", "DEBUG"),
    tracing::Level::TRACE => ("\x1b[35m", "TRACE"),
  };
  write!(writer, "{color}{label}\x1b[0m")
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
