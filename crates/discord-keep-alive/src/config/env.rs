use super::file::{
  FileAccount, FileActivity, FileClientProperties, FileConfig, FileCustomStatus,
  any_activity_field_set, any_custom_status_field_set,
};

pub fn apply_flat_env_overrides(file: &mut FileConfig) {
  if let Ok(v) = std::env::var("LOG_LEVEL")
    && !v.is_empty()
  {
    file.log_level = v;
  }

  fill_client_properties_from_env(&mut file.defaults.bot, "DEFAULTS_BOT_");
  fill_client_properties_from_env(&mut file.defaults.web, "DEFAULTS_WEB_");
  fill_client_properties_from_env(&mut file.defaults.desktop, "DEFAULTS_DESKTOP_");
  fill_client_properties_from_env(&mut file.defaults.mobile, "DEFAULTS_MOBILE_");

  fill_env_opts([
    (&mut file.account.token, "TOKEN"),
    (&mut file.account.name, "NAME"),
    (&mut file.account.kind, "KIND"),
    (&mut file.account.device, "DEVICE"),
    (&mut file.account.status, "STATUS"),
  ]);

  fill_custom_status_from_env(&mut file.account, "");
  fill_singular_activity_from_env(&mut file.account, "");
  apply_indexed_activities(&mut file.account.activities, "ACTIVITY_");

  apply_indexed_account_env(file);
}

fn fill_client_properties_from_env(set: &mut FileClientProperties, prefix: &str) {
  fill_env_opts([
    (&mut set.os, format!("{prefix}OS")),
    (&mut set.browser, format!("{prefix}BROWSER")),
    (&mut set.device, format!("{prefix}DEVICE")),
    (&mut set.user_agent, format!("{prefix}USER_AGENT")),
  ]);
}

fn apply_indexed_account_env(file: &mut FileConfig) {
  for index in collect_indices("ACCOUNT_", "_TOKEN") {
    let prefix = format!("ACCOUNT_{index}_");
    let mut acc = FileAccount {
      token: env_opt(format!("{prefix}TOKEN")),
      ..Default::default()
    };
    if acc.token.as_ref().is_none_or(|t| t.is_empty()) {
      continue;
    }

    fill_env_opts([
      (&mut acc.name, format!("{prefix}NAME")),
      (&mut acc.kind, format!("{prefix}KIND")),
      (&mut acc.device, format!("{prefix}DEVICE")),
      (&mut acc.status, format!("{prefix}STATUS")),
    ]);

    fill_custom_status_from_env(&mut acc, &prefix);
    fill_singular_activity_from_env(&mut acc, &prefix);
    apply_indexed_activities(&mut acc.activities, &format!("{prefix}ACTIVITY_"));

    insert_indexed_account(&mut file.accounts, index, acc);
  }
}

fn fill_custom_status_from_env(account: &mut FileAccount, prefix: &str) {
  let mut custom = account.custom_status.take().unwrap_or_default();
  fill_env_opts([
    (&mut custom.text, format!("{prefix}CUSTOM_STATUS_TEXT")),
    (&mut custom.emoji, format!("{prefix}CUSTOM_STATUS_EMOJI")),
  ]);
  if any_custom_status_field_set(&custom) {
    account.custom_status = Some(custom);
  }
}

fn fill_singular_activity_from_env(account: &mut FileAccount, prefix: &str) {
  let mut act = account.activity.take().unwrap_or_default();
  fill_activity_from_env(
    &mut act,
    &format!("{prefix}ACTIVITY"),
    &format!("{prefix}ACTIVITY_"),
  );
  if any_activity_field_set(&act) {
    account.activity = Some(act);
  }
}

// Discover by activity name (`ACTIVITY_0`), same role as `ACCOUNT_0_TOKEN`.
// Fields hang off that index (`ACTIVITY_0_TYPE`, ...).
fn apply_indexed_activities(activities: &mut Vec<FileActivity>, base: &str) {
  for index in collect_indices(base, "") {
    let name_key = format!("{base}{index}");
    let field_prefix = format!("{base}{index}_");
    let mut act = FileActivity::default();
    fill_activity_from_env(&mut act, &name_key, &field_prefix);
    if any_activity_field_set(&act) {
      insert_indexed_activity(activities, index, act);
    }
  }
}

fn fill_activity_from_env(act: &mut FileActivity, name_key: &str, field_prefix: &str) {
  fill_env_opts([
    (&mut act.name, name_key.to_string()),
    (&mut act.activity_type, format!("{field_prefix}TYPE")),
    (&mut act.platform, format!("{field_prefix}PLATFORM")),
    (&mut act.timestamp, format!("{field_prefix}TIMESTAMP")),
    (
      &mut act.application_id,
      format!("{field_prefix}APPLICATION_ID"),
    ),
    (&mut act.details, format!("{field_prefix}DETAILS")),
    (&mut act.url, format!("{field_prefix}URL")),
    (&mut act.large_image, format!("{field_prefix}LARGE_IMAGE")),
    (
      &mut act.large_image_text,
      format!("{field_prefix}LARGE_IMAGE_TEXT"),
    ),
    (&mut act.small_image, format!("{field_prefix}SMALL_IMAGE")),
    (
      &mut act.small_image_text,
      format!("{field_prefix}SMALL_IMAGE_TEXT"),
    ),
    (&mut act.button, format!("{field_prefix}BUTTON")),
    (&mut act.button_url, format!("{field_prefix}BUTTON_URL")),
    (&mut act.button2, format!("{field_prefix}BUTTON_2")),
    (&mut act.button2_url, format!("{field_prefix}BUTTON_2_URL")),
    (&mut act.party_id, format!("{field_prefix}PARTY_ID")),
    (
      &mut act.party_current,
      format!("{field_prefix}PARTY_CURRENT"),
    ),
    (&mut act.party_max, format!("{field_prefix}PARTY_MAX")),
  ]);
}

fn insert_indexed_account(accounts: &mut Vec<FileAccount>, index: usize, acc: FileAccount) {
  if index < accounts.len() {
    merge_account(&mut accounts[index], acc);
  } else {
    accounts.push(acc);
  }
}

// Pad gaps so ACTIVITY_5 stays at index 5 (not compacted on merge).
fn insert_indexed_activity(activities: &mut Vec<FileActivity>, index: usize, act: FileActivity) {
  if index < activities.len() {
    merge_activity(&mut activities[index], act);
  } else {
    activities.resize_with(index + 1, FileActivity::default);
    activities[index] = act;
  }
}

// Keys shaped `{prefix}{index}{suffix}` with a non-empty value.
// Empty suffix matches bare indices like `ACTIVITY_0` (not `ACTIVITY_0_TYPE`).
fn collect_indices(prefix: &str, suffix: &str) -> Vec<usize> {
  let mut found: Vec<usize> = std::env::vars()
    .filter_map(|(key, value)| {
      if value.is_empty() {
        return None;
      }
      parse_indexed_key(&key, prefix, suffix)
    })
    .collect();
  found.sort_unstable();
  found.dedup();
  found
}

fn parse_indexed_key(key: &str, prefix: &str, suffix: &str) -> Option<usize> {
  let rest = key.strip_prefix(prefix)?;
  let index_str = if suffix.is_empty() {
    rest
  } else {
    rest.strip_suffix(suffix)?
  };
  if index_str.is_empty() || !index_str.bytes().all(|b| b.is_ascii_digit()) {
    return None;
  }
  if index_str.len() > 1 && index_str.starts_with('0') {
    return None;
  }
  index_str.parse().ok()
}

fn env_opt(key: impl AsRef<str>) -> Option<String> {
  std::env::var(key.as_ref())
    .ok()
    .map(|v| v.trim().to_string())
    .filter(|v| !v.is_empty())
}

fn fill_env_opts<'a, K>(pairs: impl IntoIterator<Item = (&'a mut Option<String>, K)>)
where
  K: AsRef<str>,
{
  for (dst, key) in pairs {
    if let Some(v) = env_opt(key.as_ref()) {
      *dst = Some(v);
    }
  }
}

fn merge_opts<'a, T: 'a>(pairs: impl IntoIterator<Item = (&'a mut Option<T>, Option<T>)>) {
  for (dst, src) in pairs {
    if src.is_some() {
      *dst = src;
    }
  }
}

fn merge_account(dst: &mut FileAccount, src: FileAccount) {
  merge_opts([
    (&mut dst.name, src.name),
    (&mut dst.token, src.token),
    (&mut dst.kind, src.kind),
    (&mut dst.device, src.device),
    (&mut dst.status, src.status),
  ]);
  if let Some(src_cs) = src.custom_status {
    merge_custom_status(
      dst
        .custom_status
        .get_or_insert_with(FileCustomStatus::default),
      src_cs,
    );
  }
  if let Some(src_act) = src.activity {
    merge_activity(
      dst.activity.get_or_insert_with(FileActivity::default),
      src_act,
    );
  }
  for (i, act) in src.activities.into_iter().enumerate() {
    if any_activity_field_set(&act) {
      insert_indexed_activity(&mut dst.activities, i, act);
    }
  }
}

fn merge_custom_status(dst: &mut FileCustomStatus, src: FileCustomStatus) {
  merge_opts([(&mut dst.text, src.text), (&mut dst.emoji, src.emoji)]);
}

fn merge_activity(dst: &mut FileActivity, src: FileActivity) {
  merge_opts([
    (&mut dst.name, src.name),
    (&mut dst.activity_type, src.activity_type),
    (&mut dst.platform, src.platform),
    (&mut dst.timestamp, src.timestamp),
    (&mut dst.application_id, src.application_id),
    (&mut dst.details, src.details),
    (&mut dst.url, src.url),
    (&mut dst.large_image, src.large_image),
    (&mut dst.large_image_text, src.large_image_text),
    (&mut dst.small_image, src.small_image),
    (&mut dst.small_image_text, src.small_image_text),
    (&mut dst.button, src.button),
    (&mut dst.button_url, src.button_url),
    (&mut dst.button2, src.button2),
    (&mut dst.button2_url, src.button2_url),
    (&mut dst.party_id, src.party_id),
    (&mut dst.party_current, src.party_current),
    (&mut dst.party_max, src.party_max),
  ]);
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn merge_activity_is_field_level() {
    let mut dst = FileAccount {
      activity: Some(FileActivity {
        name: Some("keep".into()),
        details: Some("old".into()),
        ..Default::default()
      }),
      ..Default::default()
    };
    merge_account(
      &mut dst,
      FileAccount {
        activity: Some(FileActivity {
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
    let mut dst = FileAccount {
      custom_status: Some(FileCustomStatus {
        text: Some("keep".into()),
        emoji: Some("old".into()),
      }),
      ..Default::default()
    };
    merge_account(
      &mut dst,
      FileAccount {
        custom_status: Some(FileCustomStatus {
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
  fn parse_indexed_key_account_token() {
    assert_eq!(
      parse_indexed_key("ACCOUNT_0_TOKEN", "ACCOUNT_", "_TOKEN"),
      Some(0)
    );
    assert_eq!(
      parse_indexed_key("ACCOUNT_31_TOKEN", "ACCOUNT_", "_TOKEN"),
      Some(31)
    );
    assert_eq!(
      parse_indexed_key("ACCOUNT_100_TOKEN", "ACCOUNT_", "_TOKEN"),
      Some(100)
    );
  }

  #[test]
  fn parse_indexed_key_activity_name() {
    assert_eq!(parse_indexed_key("ACTIVITY_0", "ACTIVITY_", ""), Some(0));
    assert_eq!(parse_indexed_key("ACTIVITY_12", "ACTIVITY_", ""), Some(12));
    assert_eq!(parse_indexed_key("ACTIVITY_0_TYPE", "ACTIVITY_", ""), None);
    assert_eq!(
      parse_indexed_key("ACTIVITY_0_DETAILS", "ACTIVITY_", ""),
      None
    );
  }

  #[test]
  fn parse_indexed_key_account_activity() {
    assert_eq!(
      parse_indexed_key("ACCOUNT_0_ACTIVITY_1", "ACCOUNT_0_ACTIVITY_", ""),
      Some(1)
    );
    assert_eq!(
      parse_indexed_key("ACCOUNT_0_ACTIVITY_1_TYPE", "ACCOUNT_0_ACTIVITY_", ""),
      None
    );
  }

  #[test]
  fn parse_indexed_key_rejects_noise() {
    assert_eq!(
      parse_indexed_key("ACCOUNT__TOKEN", "ACCOUNT_", "_TOKEN"),
      None
    );
    assert_eq!(
      parse_indexed_key("ACCOUNT_01_TOKEN", "ACCOUNT_", "_TOKEN"),
      None
    );
    assert_eq!(
      parse_indexed_key("ACCOUNT_x_TOKEN", "ACCOUNT_", "_TOKEN"),
      None
    );
    assert_eq!(
      parse_indexed_key("ACCOUNT_0_NAME", "ACCOUNT_", "_TOKEN"),
      None
    );
    assert_eq!(parse_indexed_key("TOKEN", "ACCOUNT_", "_TOKEN"), None);
    assert_eq!(parse_indexed_key("ACTIVITY_", "ACTIVITY_", ""), None);
    assert_eq!(parse_indexed_key("ACTIVITY_01", "ACTIVITY_", ""), None);
  }

  #[test]
  fn insert_indexed_merges_in_range() {
    let mut accounts = vec![
      FileAccount {
        name: Some("a".into()),
        ..Default::default()
      },
      FileAccount {
        name: Some("b".into()),
        ..Default::default()
      },
    ];
    insert_indexed_account(
      &mut accounts,
      1,
      FileAccount {
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
    let mut accounts = vec![FileAccount {
      name: Some("only".into()),
      ..Default::default()
    }];
    insert_indexed_account(
      &mut accounts,
      5,
      FileAccount {
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
    let mut activities = vec![FileActivity {
      name: Some("a".into()),
      ..Default::default()
    }];
    insert_indexed_activity(
      &mut activities,
      0,
      FileActivity {
        details: Some("merged".into()),
        ..Default::default()
      },
    );
    assert_eq!(activities[0].name.as_deref(), Some("a"));
    assert_eq!(activities[0].details.as_deref(), Some("merged"));

    insert_indexed_activity(
      &mut activities,
      3,
      FileActivity {
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
    let mut dst = FileAccount {
      activities: vec![FileActivity {
        name: Some("keep".into()),
        details: Some("old".into()),
        ..Default::default()
      }],
      ..Default::default()
    };
    merge_account(
      &mut dst,
      FileAccount {
        activities: vec![FileActivity {
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
    let mut dst = FileAccount {
      activities: vec![
        FileActivity {
          name: Some("zero".into()),
          details: Some("old".into()),
          ..Default::default()
        },
        FileActivity {
          name: Some("one".into()),
          ..Default::default()
        },
      ],
      ..Default::default()
    };
    let mut src_acts = vec![FileActivity::default(); 6];
    src_acts[5] = FileActivity {
      name: Some("five".into()),
      ..Default::default()
    };
    merge_account(
      &mut dst,
      FileAccount {
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
