fn main() {
  // Define `physics` cfg alias for "any physics backend is enabled".
  // Eliminates repetitive `#[cfg(any(feature = "avian2d", feature =
  // "rapier2d"))]`.
  println!("cargo:rustc-check-cfg=cfg(physics)");
  if cfg!(any(feature = "avian2d", feature = "rapier2d")) {
    println!("cargo:rustc-cfg=physics");
  }
}
