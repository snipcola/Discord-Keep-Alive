//! Test-only helpers.

use std::collections::{BTreeMap, HashMap};

use clap::Parser;

use crate::model::partial::{AccountScalars, PartialAccount, PartialActivity, PartialConfig};
use crate::schema::fields::{
  AccountScalarField, ActivityField, ClientPropField, CustomStatusField, DefaultsProfile,
};
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
  AccountScalars {
    token: Some(token.into()),
    ..Default::default()
  }
  .into()
}

pub fn named_account(name: &str) -> PartialAccount {
  AccountScalars {
    name: Some(name.into()),
    ..Default::default()
  }
  .into()
}

pub fn account_with(token: &str, tweak: impl FnOnce(&mut PartialAccount)) -> PartialAccount {
  let mut account = account_with_token(token);
  tweak(&mut account);
  account
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
    scalars: AccountScalars {
      token: Some("t".into()),
      ..Default::default()
    },
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

/// Every defaults profile × client-prop leaf (no flat CLI by design).
pub fn for_each_defaults_field(mut on_field: impl FnMut(DefaultsProfile, ClientPropField)) {
  for &profile in DefaultsProfile::ALL {
    for &field in ClientPropField::ALL {
      on_field(profile, field);
    }
  }
}

/// Stable sample value for catalog reachability (not domain-valid).
pub fn catalog_sample_value(label: &str) -> String {
  format!("cat-{label}")
}
