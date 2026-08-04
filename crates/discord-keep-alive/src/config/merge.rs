use super::partial::{
  PartialAccount, PartialActivity, PartialClientProperties, PartialConfig, PartialCustomStatus,
  PartialDefaults, any_account_field_set, any_activity_field_set, any_client_prop_set,
};
use super::schema::{
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

pub fn merge_custom_status(dst: &mut PartialCustomStatus, src: PartialCustomStatus) {
  merge_set_fields(dst, src, CustomStatusField::ALL, |f, t| f.get_mut(t));
}

pub fn merge_activity(dst: &mut PartialActivity, src: PartialActivity) {
  merge_set_fields(dst, src, ActivityField::ALL, |f, t| f.get_mut(t));
}

pub fn merge_client_properties(dst: &mut PartialClientProperties, src: PartialClientProperties) {
  merge_set_fields(dst, src, ClientPropField::ALL, |f, t| f.get_mut(t));
}

pub fn merge_defaults(dst: &mut PartialDefaults, src: PartialDefaults) {
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

pub fn merge_account(dst: &mut PartialAccount, mut src: PartialAccount) {
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
  if let Some(src_act) = src.activity {
    merge_activity(
      dst.activity.get_or_insert_with(PartialActivity::default),
      src_act,
    );
  }
  for (i, act) in src.activities.into_iter().enumerate() {
    if any_activity_field_set(&act) {
      insert_indexed_activity(&mut dst.activities, i, act);
    }
  }
}

// Overlay only fields that are set. Indexed accounts merge by position.
pub fn merge_partial(dst: &mut PartialConfig, src: PartialConfig) {
  if src.log_level.is_some() {
    dst.log_level = src.log_level;
  }
  if src.health_socket.is_some() {
    dst.health_socket = src.health_socket;
  }
  merge_defaults(&mut dst.defaults, src.defaults);
  if any_account_field_set(&src.account) {
    merge_account(&mut dst.account, src.account);
  }
  for (i, acc) in src.accounts.into_iter().enumerate() {
    if any_account_field_set(&acc) {
      insert_indexed_account(&mut dst.accounts, i, acc);
    }
  }
}

// Merge in place when the index exists; otherwise append (sparse gaps are lost).
pub fn insert_indexed_account(
  accounts: &mut Vec<PartialAccount>,
  index: usize,
  acc: PartialAccount,
) {
  if index < accounts.len() {
    merge_account(&mut accounts[index], acc);
  } else {
    accounts.push(acc);
  }
}

// Merge in place when the index exists; otherwise pad so ACTIVITY_5 stays slot 5.
// Empty pads are dropped at resolve; later layers can still fill them by index.
pub fn insert_indexed_activity(
  activities: &mut Vec<PartialActivity>,
  index: usize,
  act: PartialActivity,
) {
  if index < activities.len() {
    merge_activity(&mut activities[index], act);
  } else {
    activities.resize_with(index + 1, PartialActivity::default);
    activities[index] = act;
  }
}

#[cfg(test)]
mod tests {
  use super::super::partial::{PartialAccount, PartialActivity, PartialCustomStatus};
  use super::*;

  fn named_account(name: &str) -> PartialAccount {
    PartialAccount {
      name: Some(name.into()),
      ..Default::default()
    }
  }

  fn named_activity(name: &str) -> PartialActivity {
    PartialActivity {
      name: Some(name.into()),
      ..Default::default()
    }
  }

  fn activity_details(details: &str) -> PartialActivity {
    PartialActivity {
      details: Some(details.into()),
      ..Default::default()
    }
  }

  #[test]
  fn merge_nested_is_field_level() {
    let mut dst = PartialAccount {
      activity: Some(PartialActivity {
        name: Some("keep".into()),
        details: Some("old".into()),
        ..Default::default()
      }),
      custom_status: Some(PartialCustomStatus {
        text: Some("keep".into()),
        emoji: Some("old".into()),
      }),
      ..Default::default()
    };
    merge_account(
      &mut dst,
      PartialAccount {
        activity: Some(activity_details("new")),
        custom_status: Some(PartialCustomStatus {
          text: None,
          emoji: Some("new".into()),
        }),
        ..Default::default()
      },
    );
    let act = dst.activity.unwrap();
    assert_eq!(act.name.as_deref(), Some("keep"), "activity");
    assert_eq!(act.details.as_deref(), Some("new"), "activity");
    let cs = dst.custom_status.unwrap();
    assert_eq!(cs.text.as_deref(), Some("keep"), "custom_status");
    assert_eq!(cs.emoji.as_deref(), Some("new"), "custom_status");
  }

  #[test]
  fn insert_indexed_account_merge_and_append() {
    let mut accounts = vec![named_account("a"), named_account("b")];
    insert_indexed_account(
      &mut accounts,
      1,
      PartialAccount {
        token: Some("t".into()),
        ..Default::default()
      },
    );
    assert_eq!(accounts.len(), 2, "in_range");
    assert_eq!(accounts[1].token.as_deref(), Some("t"), "in_range");
    assert_eq!(accounts[1].name.as_deref(), Some("b"), "in_range");

    let mut accounts = vec![named_account("only")];
    insert_indexed_account(
      &mut accounts,
      5,
      PartialAccount {
        token: Some("t".into()),
        name: Some("extra".into()),
        ..Default::default()
      },
    );
    assert_eq!(accounts.len(), 2, "beyond");
    assert_eq!(accounts[0].name.as_deref(), Some("only"), "beyond");
    assert_eq!(accounts[1].name.as_deref(), Some("extra"), "beyond");
    assert_eq!(accounts[1].token.as_deref(), Some("t"), "beyond");
  }

  #[test]
  fn insert_indexed_activity_merges_and_pads() {
    let mut activities = vec![named_activity("a")];
    insert_indexed_activity(&mut activities, 0, activity_details("merged"));
    assert_eq!(activities[0].name.as_deref(), Some("a"));
    assert_eq!(activities[0].details.as_deref(), Some("merged"));
    insert_indexed_activity(&mut activities, 3, named_activity("b"));
    assert_eq!(activities.len(), 4);
    assert_eq!(activities[3].name.as_deref(), Some("b"));
    assert!(!any_activity_field_set(&activities[1]) && !any_activity_field_set(&activities[2]));
  }

  #[test]
  fn merge_account_activities_by_index() {
    let mut dst = PartialAccount {
      activities: vec![PartialActivity {
        name: Some("keep".into()),
        details: Some("old".into()),
        ..Default::default()
      }],
      ..Default::default()
    };
    merge_account(
      &mut dst,
      PartialAccount {
        activities: vec![activity_details("new")],
        ..Default::default()
      },
    );
    assert_eq!(dst.activities.len(), 1);
    assert_eq!(dst.activities[0].name.as_deref(), Some("keep"));
    assert_eq!(dst.activities[0].details.as_deref(), Some("new"));

    let mut dst = PartialAccount {
      activities: vec![
        PartialActivity {
          name: Some("zero".into()),
          details: Some("old".into()),
          ..Default::default()
        },
        named_activity("one"),
      ],
      ..Default::default()
    };
    let mut src_acts = vec![PartialActivity::default(); 6];
    src_acts[5] = named_activity("five");
    merge_account(
      &mut dst,
      PartialAccount {
        activities: src_acts,
        ..Default::default()
      },
    );
    assert_eq!(dst.activities.len(), 6, "sparse");
    assert_eq!(dst.activities[0].name.as_deref(), Some("zero"), "sparse");
    assert_eq!(dst.activities[0].details.as_deref(), Some("old"), "sparse");
    assert_eq!(dst.activities[1].name.as_deref(), Some("one"), "sparse");
    assert_eq!(dst.activities[5].name.as_deref(), Some("five"), "sparse");
  }
}
