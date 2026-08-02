use tracing::Subscriber;
use tracing::span::{Attributes, Id};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

use super::util::AccountField;

/// Captures `account` on spans so log lines can be prefixed with the account name.
pub(super) struct AccountLayer;

#[derive(Clone)]
pub(super) struct AccountName(pub String);

impl<S> Layer<S> for AccountLayer
where
  S: Subscriber + for<'a> LookupSpan<'a>,
{
  fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
    let mut visitor = AccountField::default();
    attrs.record(&mut visitor);
    if let Some(name) = visitor.account
      && let Some(span) = ctx.span(id)
    {
      span.extensions_mut().insert(AccountName(name));
    }
  }
}
