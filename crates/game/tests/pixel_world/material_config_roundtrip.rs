use game::pixel_world::material::{Materials, MaterialsConfig};

#[test]
fn builtin_config_roundtrip_via_toml() {
  let config = MaterialsConfig::builtin();
  let toml_str = toml::to_string_pretty(&config).unwrap();
  let deserialized: MaterialsConfig = toml::from_str(&toml_str).unwrap();
  let materials = Materials::from(deserialized);

  let defaults = Materials::new();
  assert_eq!(materials.len(), defaults.len());
  for i in 0..defaults.len() {
    let id = game::pixel_world::coords::MaterialId(i as u8);
    assert_eq!(materials.get(id).name, defaults.get(id).name);
    assert_eq!(materials.get(id).state, defaults.get(id).state);
    assert_eq!(materials.get(id).density, defaults.get(id).density);
  }
}
