fn main() {
  // Define `physics` cfg alias - always enabled since rapier2d is always on.
  println!("cargo:rustc-check-cfg=cfg(physics)");
  println!("cargo:rustc-cfg=physics");
}
