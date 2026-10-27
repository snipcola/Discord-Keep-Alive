use std::fmt::Display;
use std::io::IsTerminal;

pub const BOLD: &str = "1";
pub const DIM: &str = "2";
pub const RED_BOLD: &str = "1;31";

pub fn ansi_enabled() -> bool {
  std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

pub fn paint(ansi: bool, style: &str, text: impl Display) -> String {
  if ansi {
    format!("\x1b[{style}m{text}\x1b[0m")
  } else {
    text.to_string()
  }
}

pub fn report_error(err: impl Display) {
  eprintln!("{} {err}", paint(ansi_enabled(), RED_BOLD, "error:"));
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn paint_wraps_text_only_when_ansi_is_enabled() {
    assert_eq!(paint(true, BOLD, "main"), "\x1b[1mmain\x1b[0m");
    assert_eq!(paint(false, BOLD, "main"), "main");
  }
}
