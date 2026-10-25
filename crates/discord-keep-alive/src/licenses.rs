use std::io::Write;

macro_rules! workspace_file {
  ($($part:expr),+) => {
    include_str!(concat!(env!("CARGO_WORKSPACE_DIR"), $($part),+))
  };
}

const LICENSES: &str = concat!(
  workspace_file!("LICENSE"),
  "\n",
  workspace_file!("licenses.md"),
);

pub fn print() {
  let _ = std::io::stdout().write_all(LICENSES.as_bytes());
}
