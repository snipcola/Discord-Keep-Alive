use super::partial::{
  PartialAccount, PartialActivity, PartialClientProperties, PartialConfig, PartialCustomStatus,
  PartialDefaults, any_account_field_set, any_activity_field_set, any_client_prop_set,
};
use super::schema::{AccountScalarField, ActivityField, ClientPropField, CustomStatusField};

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
  if any_client_prop_set(&src.bot) {
    merge_client_properties(&mut dst.bot, src.bot);
  }
  if any_client_prop_set(&src.web) {
    merge_client_properties(&mut dst.web, src.web);
  }
  if any_client_prop_set(&src.desktop) {
    merge_client_properties(&mut dst.desktop, src.desktop);
  }
  if any_client_prop_set(&src.mobile) {
    merge_client_properties(&mut dst.mobile, src.mobile);
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

/// Set fields only; indexed accounts merge by vector index.
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

/// In-range: merge. Beyond len: append without padding (gaps lose absolute index).
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

/// In-range: merge. Beyond len: pad so the index stays absolute (e.g. `ACTIVITY_5` → slot 5).
/// Name-less pads drop at resolve; later layers can still overlay by index.
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

  #[test]
  fn merge_activity_is_field_level() {
    let mut dst = PartialAccount {
      activity: Some(PartialActivity {
        name: Some("keep".into()),
        details: Some("old".into()),
        ..Default::default()
      }),
      ..Default::default()
    };
    merge_account(
      &mut dst,
      PartialAccount {
        activity: Some(PartialActivity {
          details: Some("new".into()),
          ..Default::default()
        }),
        ..Default::default()
      },
    );
    let act = dst.activity.unwrap();
    assert_eq!(act.name.as_deref(), Some("keep"));
    assert_eq!(act.details.as_deref(), Some("new"));
  }

  #[test]
  fn merge_custom_status_is_field_level() {
    let mut dst = PartialAccount {
      custom_status: Some(PartialCustomStatus {
        text: Some("keep".into()),
        emoji: Some("old".into()),
      }),
      ..Default::default()
    };
    merge_account(
      &mut dst,
      PartialAccount {
        custom_status: Some(PartialCustomStatus {
          text: None,
          emoji: Some("new".into()),
        }),
        ..Default::default()
      },
    );
    let cs = dst.custom_status.unwrap();
    assert_eq!(cs.text.as_deref(), Some("keep"));
    assert_eq!(cs.emoji.as_deref(), Some("new"));
  }

  #[test]
  fn insert_indexed_merges_in_range() {
    let mut accounts = vec![
      PartialAccount {
        name: Some("a".into()),
        ..Default::default()
      },
      PartialAccount {
        name: Some("b".into()),
        ..Default::default()
      },
    ];
    insert_indexed_account(
      &mut accounts,
      1,
      PartialAccount {
        token: Some("t".into()),
        ..Default::default()
      },
    );
    assert_eq!(accounts.len(), 2);
    assert_eq!(accounts[1].token.as_deref(), Some("t"));
    assert_eq!(accounts[1].name.as_deref(), Some("b"));
  }

  #[test]
  fn insert_indexed_appends_beyond_len_without_padding() {
    let mut accounts = vec![PartialAccount {
      name: Some("only".into()),
      ..Default::default()
    }];
    insert_indexed_account(
      &mut accounts,
      5,
      PartialAccount {
        token: Some("t".into()),
        name: Some("extra".into()),
        ..Default::default()
      },
    );
    assert_eq!(accounts.len(), 2);
    assert_eq!(accounts[0].name.as_deref(), Some("only"));
    assert_eq!(accounts[1].name.as_deref(), Some("extra"));
    assert_eq!(accounts[1].token.as_deref(), Some("t"));
  }

  #[test]
  fn insert_indexed_activity_merges_and_pads() {
    let mut activities = vec![PartialActivity {
      name: Some("a".into()),
      ..Default::default()
    }];
    insert_indexed_activity(
      &mut activities,
      0,
      PartialActivity {
        details: Some("merged".into()),
        ..Default::default()
      },
    );
    assert_eq!(activities[0].name.as_deref(), Some("a"));
    assert_eq!(activities[0].details.as_deref(), Some("merged"));

    insert_indexed_activity(
      &mut activities,
      3,
      PartialActivity {
        name: Some("b".into()),
        ..Default::default()
      },
    );
    assert_eq!(activities.len(), 4);
    assert_eq!(activities[3].name.as_deref(), Some("b"));
    assert!(!any_activity_field_set(&activities[1]));
    assert!(!any_activity_field_set(&activities[2]));
  }

  #[test]
  fn merge_account_merges_activities_by_index() {
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
        activities: vec![PartialActivity {
          details: Some("new".into()),
          ..Default::default()
        }],
        ..Default::default()
      },
    );
    assert_eq!(dst.activities.len(), 1);
    assert_eq!(dst.activities[0].name.as_deref(), Some("keep"));
    assert_eq!(dst.activities[0].details.as_deref(), Some("new"));
  }

  #[test]
  fn merge_account_preserves_sparse_activity_indices() {
    let mut dst = PartialAccount {
      activities: vec![
        PartialActivity {
          name: Some("zero".into()),
          details: Some("old".into()),
          ..Default::default()
        },
        PartialActivity {
          name: Some("one".into()),
          ..Default::default()
        },
      ],
      ..Default::default()
    };
    let mut src_acts = vec![PartialActivity::default(); 6];
    src_acts[5] = PartialActivity {
      name: Some("five".into()),
      ..Default::default()
    };
    merge_account(
      &mut dst,
      PartialAccount {
        activities: src_acts,
        ..Default::default()
      },
    );
    assert_eq!(dst.activities.len(), 6);
    assert_eq!(dst.activities[0].name.as_deref(), Some("zero"));
    assert_eq!(dst.activities[0].details.as_deref(), Some("old"));
    assert_eq!(dst.activities[1].name.as_deref(), Some("one"));
    assert_eq!(dst.activities[5].name.as_deref(), Some("five"));
  }
}
