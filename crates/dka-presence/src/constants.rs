use std::fmt;

macro_rules! string_enum {
  (
    $(#[$enum_meta:meta])*
    $vis:vis enum $name:ident {
      $(
        $(#[$var_meta:meta])*
        $variant:ident $(= $disc:expr)? => $str:literal
      ),+ $(,)?
    }
  ) => {
    string_enum! {
      $(#[$enum_meta])*
      $vis enum $name {
        $(
          $(#[$var_meta])*
          $variant $(= $disc)? => $str
        ),+
      }
      all = [$($variant),+]
    }
  };
  (
    $(#[$enum_meta:meta])*
    $vis:vis enum $name:ident {
      $(
        $(#[$var_meta:meta])*
        $variant:ident $(= $disc:expr)? => $str:literal
      ),+ $(,)?
    }
    all = [$($all:ident),+ $(,)?]
  ) => {
    $(#[$enum_meta])*
    $vis enum $name {
      $(
        $(#[$var_meta])*
        $variant $(= $disc)?,
      )+
    }

    impl $name {
      pub const ALL: &'static [Self] = &[$(Self::$all),+];

      pub fn as_str(self) -> &'static str {
        match self {
          $(Self::$variant => $str,)+
        }
      }

      pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
          .iter()
          .copied()
          .find(|v| v.as_str().eq_ignore_ascii_case(value))
      }
    }

    impl fmt::Display for $name {
      fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
      }
    }
  };
}

string_enum! {
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
  pub enum AccountKind {
    #[default]
    User => "user",
    Bot => "bot",
  }
}

string_enum! {
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub enum Device {
    Web => "web",
    Desktop => "desktop",
    Mobile => "mobile",
  }
}

string_enum! {
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub enum Status {
    Online => "online",
    Idle => "idle",
    Invisible => "invisible",
    Dnd => "dnd",
  }
}

string_enum! {
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub enum ActivityType {
    Playing = 0 => "PLAYING",
    Streaming = 1 => "STREAMING",
    Listening = 2 => "LISTENING",
    Watching = 3 => "WATCHING",
    Custom = 4 => "CUSTOM",
    Competing = 5 => "COMPETING",
    Hang = 6 => "HANG",
  }
  all = [Custom, Playing, Streaming, Listening, Watching, Competing, Hang]
}

impl ActivityType {
  pub fn as_i64(self) -> i64 {
    self as i64
  }
}

string_enum! {
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub enum ActivityPlatform {
    Desktop => "desktop",
    Samsung => "samsung",
    Xbox => "xbox",
    Ios => "ios",
    Android => "android",
    Embedded => "embedded",
    Ps4 => "ps4",
    Ps5 => "ps5",
  }
}

pub(crate) const DEFAULT_APPLICATION_ID: &str = "1";
pub(crate) const DEFAULT_PARTY_ID: &str = "1";
