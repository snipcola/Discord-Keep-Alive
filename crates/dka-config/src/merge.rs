use std::collections::BTreeMap;

use crate::model::partial::{
  PartialAccount, PartialActivity, PartialClientProperties, PartialConfig, PartialCustomStatus,
  PartialDefaults, any_account_field_set, any_activity_field_set, any_client_prop_set,
};
use crate::schema::fields::{
  AccountScalarField, ActivityField, ClientPropField, CustomStatusField, DefaultsProfile,
};

fn merge_set_fields<F, T>(
  dst: &mut T,
  mut src: T,
  fields: &[F],
  get_mut: impl Fn(F, &mut T) -> &mut Option<String>,
) where
  F: Copy,
{
  for &field in fields {
    let v = get_mut(field, &mut src).take();
    if v.is_some() {
      *get_mut(field, dst) = v;
    }
  }
}

pub(crate) fn merge_custom_status(dst: &mut PartialCustomStatus, src: PartialCustomStatus) {
  merge_set_fields(dst, src, CustomStatusField::ALL, |f, t| f.get_mut(t));
}

pub(crate) fn merge_activity(dst: &mut PartialActivity, src: PartialActivity) {
  merge_set_fields(dst, src, ActivityField::ALL, |f, t| f.get_mut(t));
}

pub(crate) fn merge_client_properties(
  dst: &mut PartialClientProperties,
  src: PartialClientProperties,
) {
  merge_set_fields(dst, src, ClientPropField::ALL, |f, t| f.get_mut(t));
}

pub(crate) fn merge_defaults(dst: &mut PartialDefaults, src: PartialDefaults) {
  let PartialDefaults {
    bot,
    web,
    desktop,
    mobile,
  } = src;
  for (profile, props) in [
    (DefaultsProfile::Bot, bot),
    (DefaultsProfile::Web, web),
    (DefaultsProfile::Desktop, desktop),
    (DefaultsProfile::Mobile, mobile),
  ] {
    if any_client_prop_set(&props) {
      merge_client_properties(profile.props_mut(dst), props);
    }
  }
}

pub(crate) fn ensure_id_in_order(order: &mut Vec<String>, id: &str) {
  if !order.iter().any(|existing| existing == id) {
    order.push(id.to_string());
  }
}

pub(crate) fn append_missing_keys<'a, I>(order: &mut Vec<String>, keys: I)
where
  I: IntoIterator<Item = &'a String>,
{
  for id in keys {
    ensure_id_in_order(order, id);
  }
}

pub(crate) fn merge_order(dst: &mut Vec<String>, src: &[String]) {
  append_missing_keys(dst, src);
}

fn merge_map_by_id<V>(
  dst: &mut BTreeMap<String, V>,
  src: BTreeMap<String, V>,
  is_set: impl Fn(&V) -> bool,
  merge: impl Fn(&mut V, V),
) {
  for (id, val) in src {
    if !is_set(&val) {
      continue;
    }
    match dst.get_mut(&id) {
      Some(existing) => merge(existing, val),
      None => {
        dst.insert(id, val);
      }
    }
  }
}

pub(crate) fn merge_account(dst: &mut PartialAccount, mut src: PartialAccount) {
  for &field in AccountScalarField::ALL {
    if let Some(v) = field.take(&mut src) {
      field.set(dst, v);
    }
  }
  if let Some(src_cs) = src.custom_status {
    merge_custom_status(
      dst
        .custom_status
        .get_or_insert_with(PartialCustomStatus::default),
      src_cs,
    );
  }
  merge_map_by_id(
    &mut dst.activities,
    src.activities,
    any_activity_field_set,
    merge_activity,
  );
  merge_order(&mut dst.activity_order, &src.activity_order);
  append_missing_keys(&mut dst.activity_order, dst.activities.keys());
}

// Field-level overlay. Maps merge by string id; order only appends missing ids.
// Ids are never renumbered: "1" stays "1".
pub(crate) fn merge_partial(dst: &mut PartialConfig, src: PartialConfig) {
  if src.log_level.is_some() {
    dst.log_level = src.log_level;
  }
  if src.health_socket.is_some() {
    dst.health_socket = src.health_socket;
  }
  merge_defaults(&mut dst.defaults, src.defaults);
  merge_map_by_id(
    &mut dst.accounts,
    src.accounts,
    any_account_field_set,
    merge_account,
  );
  merge_order(&mut dst.account_order, &src.account_order);
  append_missing_keys(&mut dst.account_order, dst.accounts.keys());
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::partial::{
    PartialAccount, PartialActivity, PartialConfig, PartialCustomStatus,
  };
  use crate::schema::fields::{ActivityField, CustomStatusField};
  use crate::test_support::*;

  fn activity_details(details: &str) -> PartialActivity {
    PartialActivity {
      details: Some(details.into()),
      ..Default::default()
    }
  }

  #[test]
  fn merge_nested_is_field_level() {
    let mut dst = PartialAccount {
      activities: BTreeMap::from([(
        "main".into(),
        PartialActivity {
          name: Some("keep".into()),
          details: Some("old".into()),
          ..Default::default()
        },
      )]),
      activity_order: vec!["main".into()],
      custom_status: Some(PartialCustomStatus {
        text: Some("keep".into()),
        emoji: Some("old".into()),
      }),
      ..Default::default()
    };
    merge_account(
      &mut dst,
      PartialAccount {
        activities: BTreeMap::from([("main".into(), activity_details("new"))]),
        activity_order: vec!["main".into()],
        custom_status: Some(PartialCustomStatus {
          text: None,
          emoji: Some("new".into()),
        }),
        ..Default::default()
      },
    );
    let act = dst.activities.get("main").unwrap();
    assert_eq!(act.name.as_deref(), Some("keep"));
    assert_eq!(act.details.as_deref(), Some("new"));
    let cs = dst.custom_status.unwrap();
    assert_eq!(cs.text.as_deref(), Some("keep"));
    assert_eq!(cs.emoji.as_deref(), Some("new"));
  }

  #[test]
  fn merge_by_id_and_order_append() {
    let mut dst = PartialConfig {
      accounts: BTreeMap::from([("a".into(), named_account("A"))]),
      account_order: vec!["a".into()],
      ..Default::default()
    };
    let src = PartialConfig {
      accounts: BTreeMap::from([
        ("a".into(), account_with_token("t")),
        ("b".into(), named_account("B")),
      ]),
      account_order: vec!["a".into(), "b".into()],
      ..Default::default()
    };
    merge_partial(&mut dst, src);
    assert_eq!(dst.account_order, vec!["a".to_string(), "b".to_string()]);
    assert_eq!(dst.accounts.get("a").unwrap().name.as_deref(), Some("A"));
    assert_eq!(dst.accounts.get("a").unwrap().token.as_deref(), Some("t"));
    assert_eq!(dst.accounts.get("b").unwrap().name.as_deref(), Some("B"));
  }

  #[test]
  fn merge_no_pack_account_id_one_stays_one() {
    let mut dst = PartialConfig::default();
    let src = PartialConfig {
      accounts: BTreeMap::from([(
        "1".into(),
        account_with("tok-1", |a| a.name = Some("one".into())),
      )]),
      account_order: vec!["1".into()],
      ..Default::default()
    };
    merge_partial(&mut dst, src);
    assert_eq!(dst.account_order, vec!["1".to_string()]);
    assert!(dst.accounts.contains_key("1"));
    assert!(!dst.accounts.contains_key("0"));
    assert_eq!(dst.accounts.len(), 1);
    assert_eq!(
      dst.accounts.get("1").unwrap().token.as_deref(),
      Some("tok-1")
    );
    assert_eq!(dst.accounts.get("1").unwrap().name.as_deref(), Some("one"));
  }

  #[test]
  fn merge_activities_by_id_no_pad() {
    let mut dst = PartialAccount {
      activities: BTreeMap::from([(
        "0".into(),
        PartialActivity {
          name: Some("zero".into()),
          details: Some("old".into()),
          ..Default::default()
        },
      )]),
      activity_order: vec!["0".into()],
      ..Default::default()
    };
    merge_account(
      &mut dst,
      PartialAccount {
        activities: BTreeMap::from([
          ("0".into(), activity_details("new")),
          ("5".into(), named_activity("five")),
        ]),
        activity_order: vec!["0".into(), "5".into()],
        ..Default::default()
      },
    );
    assert_eq!(dst.activity_order, vec!["0".to_string(), "5".to_string()]);
    assert_eq!(dst.activities.len(), 2);
    assert_eq!(
      dst.activities.get("0").unwrap().name.as_deref(),
      Some("zero")
    );
    assert_eq!(
      dst.activities.get("0").unwrap().details.as_deref(),
      Some("new")
    );
    assert_eq!(
      dst.activities.get("5").unwrap().name.as_deref(),
      Some("five")
    );
    assert!(!dst.activities.contains_key("1"));
  }

  #[test]
  fn merge_order_appends_missing_only() {
    let mut order = vec!["a".into(), "b".into()];
    merge_order(&mut order, &["b".into(), "c".into(), "a".into()]);
    assert_eq!(
      order,
      vec!["a".to_string(), "b".to_string(), "c".to_string()]
    );
  }

  /// Catalog leaves merge by field id (src None does not wipe dst).
  #[test]
  fn catalog_activity_and_custom_merge_field_level() {
    for_each_catalog_field(
      |_| {},
      |field| {
        let mut dst = PartialCustomStatus::default();
        *field.get_mut(&mut dst) = Some("keep".into());
        let mut src = PartialCustomStatus::default();
        for &other in CustomStatusField::ALL {
          if other != field {
            *other.get_mut(&mut src) = Some(format!("src-{}", other.spec().set_suffix));
          }
        }
        merge_custom_status(&mut dst, src);
        assert_eq!(
          field.get_mut(&mut dst).as_deref(),
          Some("keep"),
          "dst leaf {}",
          field.spec().set_suffix
        );
      },
      |field| {
        let mut dst = PartialActivity::default();
        *field.get_mut(&mut dst) = Some("keep".into());
        let mut src = PartialActivity::default();
        // Set a different leaf on src so merge is non-empty without wiping `field`.
        if let Some(&other) = ActivityField::ALL.iter().find(|&&f| f != field) {
          *other.get_mut(&mut src) = Some("from-src".into());
          merge_activity(&mut dst, src);
          assert_eq!(
            field.get_mut(&mut dst).as_deref(),
            Some("keep"),
            "dst leaf {}",
            field.spec().set_suffix
          );
          assert_eq!(
            other.get_mut(&mut dst).as_deref(),
            Some("from-src"),
            "src leaf {}",
            other.spec().set_suffix
          );
        }
      },
    );
  }
}
