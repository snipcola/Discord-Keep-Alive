use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::{Event, Subscriber};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FmtContext, FormatEvent};
use tracing_subscriber::registry::LookupSpan;

use super::account::AccountName;
use super::util::EventFields;

pub(super) struct HumanFormat {
  pub ansi: bool,
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
    write_hms(&mut writer)?;
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

fn write_hms(writer: &mut Writer<'_>) -> fmt::Result {
  let secs = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|d| d.as_secs())
    .unwrap_or(0);
  let days = secs / 86_400;
  let tod = secs % 86_400;
  let h = tod / 3600;
  let m = (tod % 3600) / 60;
  let s = tod % 60;
  let (year, month, day) = civil_from_days(days as i64);
  write!(
    writer,
    "{year:04}-{month:02}-{day:02} {h:02}:{m:02}:{s:02}Z"
  )
}

// Unix epoch day count -> Y-M-D (Howard civil_from_days).
fn civil_from_days(days: i64) -> (i32, u32, u32) {
  let z = days + 719_468;
  let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
  let doe = (z - era * 146_097) as u64;
  let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
  let y = yoe as i64 + era * 400;
  let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
  let mp = (5 * doy + 2) / 153;
  let d = doy - (153 * mp + 2) / 5 + 1;
  let m = if mp < 10 { mp + 3 } else { mp - 9 };
  let y = if m <= 2 { y + 1 } else { y };
  (y as i32, m as u32, d as u32)
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
  for span in scope.from_root() {
    if let Some(AccountName(name)) = span.extensions().get::<AccountName>() {
      return Some(name.clone());
    }
  }
  None
}
