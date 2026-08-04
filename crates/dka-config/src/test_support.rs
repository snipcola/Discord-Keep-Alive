//! Test-only helpers.

use std::collections::{BTreeMap, HashMap};

use clap::Parser;

use crate::model::partial::{PartialAccount, PartialActivity, PartialConfig};
use crate::schema::fields::{AccountScalarField, ActivityField, CustomStatusField};
use crate::schema::id::ACCOUNT_FLAT;
use crate::source::cli::Cli;

pub fn empty_cli() -> Cli {
  Cli::try_parse_from(["discord-keep-alive"]).expect("empty cli parse")
}

pub fn env_map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
  pairs
    .iter()
    .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
    .collect()
}

pub fn account_with_token(token: &str) -> PartialAccount {
  PartialAccount {
    token: Some(token.into()),
    ..Default::default()
  }
}

pub fn named_account(name: &str) -> PartialAccount {
  PartialAccount {
    name: Some(name.into()),
    ..Default::default()
  }
}

pub fn named_activity(name: &str) -> PartialActivity {
  PartialActivity {
    name: Some(name.into()),
    ..Default::default()
  }
}

pub fn partial_flat(account: PartialAccount) -> PartialConfig {
  PartialConfig {
    account_order: vec![ACCOUNT_FLAT.into()],
    accounts: BTreeMap::from([(ACCOUNT_FLAT.into(), account)]),
    ..Default::default()
  }
}

pub fn flat_with_activities(
  activities: BTreeMap<String, PartialActivity>,
  activity_order: Vec<String>,
) -> PartialConfig {
  partial_flat(PartialAccount {
    token: Some("t".into()),
    activities,
    activity_order,
    ..Default::default()
  })
}

pub fn token_of<'a>(p: &'a PartialConfig, id: &str) -> Option<&'a str> {
  p.accounts.get(id).and_then(|a| a.token.as_deref())
}

pub fn status_of<'a>(p: &'a PartialConfig, id: &str) -> Option<&'a str> {
  p.accounts.get(id).and_then(|a| a.status.as_deref())
}

pub fn name_of<'a>(p: &'a PartialConfig, id: &str) -> Option<&'a str> {
  p.accounts.get(id).and_then(|a| a.name.as_deref())
}

pub fn for_each_catalog_field(
  mut on_account: impl FnMut(AccountScalarField),
  mut on_custom: impl FnMut(CustomStatusField),
  mut on_activity: impl FnMut(ActivityField),
) {
  for &field in AccountScalarField::ALL {
    on_account(field);
  }
  for &field in CustomStatusField::ALL {
    on_custom(field);
  }
  for &field in ActivityField::ALL {
    on_activity(field);
  }
}
