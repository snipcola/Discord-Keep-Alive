fn main() {
  let target = std::env::var("TARGET").expect("TARGET");
  println!("cargo::rerun-if-changed=build.rs");
  println!("cargo::rustc-env=DKA_TARGET={target}");
}
