use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AccountKind {
  #[default]
  User,
  Bot,
}

impl AccountKind {
  pub const ALL: &'static [Self] = &[Self::User, Self::Bot];

  pub fn as_str(self) -> &'static str {
    match self {
      Self::User => "user",
      Self::Bot => "bot",
    }
  }

  pub fn parse(value: &str) -> Option<Self> {
    Self::ALL
      .iter()
      .copied()
      .find(|k| k.as_str().eq_ignore_ascii_case(value))
  }
}

impl fmt::Display for AccountKind {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.as_str())
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Device {
  Web,
  Desktop,
  Mobile,
}

impl Device {
  pub const ALL: &'static [Self] = &[Self::Web, Self::Desktop, Self::Mobile];

  pub fn as_str(self) -> &'static str {
    match self {
      Self::Web => "web",
      Self::Desktop => "desktop",
      Self::Mobile => "mobile",
    }
  }

  pub fn parse(value: &str) -> Option<Self> {
    Self::ALL
      .iter()
      .copied()
      .find(|d| d.as_str().eq_ignore_ascii_case(value))
  }
}

impl fmt::Display for Device {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.as_str())
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
  Online,
  Idle,
  Invisible,
  Dnd,
}

impl Status {
  pub const ALL: &'static [Self] = &[Self::Online, Self::Idle, Self::Invisible, Self::Dnd];

  pub fn as_str(self) -> &'static str {
    match self {
      Self::Online => "online",
      Self::Idle => "idle",
      Self::Invisible => "invisible",
      Self::Dnd => "dnd",
    }
  }

  pub fn parse(value: &str) -> Option<Self> {
    Self::ALL
      .iter()
      .copied()
      .find(|s| s.as_str().eq_ignore_ascii_case(value))
  }
}

impl fmt::Display for Status {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.as_str())
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActivityType {
  Playing = 0,
  Streaming = 1,
  Listening = 2,
  Watching = 3,
  Custom = 4,
  Competing = 5,
  Hang = 6,
}

impl ActivityType {
  pub const ALL: &'static [Self] = &[
    Self::Custom,
    Self::Playing,
    Self::Streaming,
    Self::Listening,
    Self::Watching,
    Self::Competing,
    Self::Hang,
  ];

  pub fn as_str(self) -> &'static str {
    match self {
      Self::Custom => "CUSTOM",
      Self::Playing => "PLAYING",
      Self::Streaming => "STREAMING",
      Self::Listening => "LISTENING",
      Self::Watching => "WATCHING",
      Self::Competing => "COMPETING",
      Self::Hang => "HANG",
    }
  }

  pub fn as_i64(self) -> i64 {
    self as i64
  }

  pub fn parse(value: &str) -> Option<Self> {
    Self::ALL
      .iter()
      .copied()
      .find(|t| t.as_str().eq_ignore_ascii_case(value))
  }
}

impl fmt::Display for ActivityType {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.as_str())
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActivityPlatform {
  Desktop,
  Samsung,
  Xbox,
  Ios,
  Android,
  Embedded,
  Ps4,
  Ps5,
}

impl ActivityPlatform {
  pub const ALL: &'static [Self] = &[
    Self::Desktop,
    Self::Samsung,
    Self::Xbox,
    Self::Ios,
    Self::Android,
    Self::Embedded,
    Self::Ps4,
    Self::Ps5,
  ];

  pub fn as_str(self) -> &'static str {
    match self {
      Self::Desktop => "desktop",
      Self::Samsung => "samsung",
      Self::Xbox => "xbox",
      Self::Ios => "ios",
      Self::Android => "android",
      Self::Embedded => "embedded",
      Self::Ps4 => "ps4",
      Self::Ps5 => "ps5",
    }
  }

  pub fn parse(value: &str) -> Option<Self> {
    Self::ALL
      .iter()
      .copied()
      .find(|p| p.as_str().eq_ignore_ascii_case(value))
  }
}

impl fmt::Display for ActivityPlatform {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.as_str())
  }
}

pub const DEFAULT_APPLICATION_ID: &str = "1";
pub const DEFAULT_PARTY_ID: &str = "1";
