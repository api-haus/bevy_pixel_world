//! Pixel packing macros for byte-aligned, shader-friendly pixel data
//! structures.
//!
//! # Design Goals
//!
//! - Byte-aligned fields (shader-friendly)
//! - Named accessors for flags and nibbles
//! - `#[repr(C)]` for predictable memory layout
//! - Zero-cost abstractions (all accessors inline)
//!
//! # Integration
//!
//! Games use these macros to define their pixel type. The type implements
//! `PixelBase` (from bevy_pixel_world), making it compatible with the
//! framework's simulation and iteration infrastructure.

/// Generate a flags struct with named bit accessors.
///
/// Creates a newtype around `u8` with getter/setter methods for each flag.
/// Flags are assigned bits 0-7 in declaration order.
///
/// # Example
///
/// ```
/// # use pixel_macro::flags8;
/// flags8!(PixelFlags {
///     dirty,      // bit 0
///     solid,      // bit 1
///     falling,    // bit 2
///     burning,    // bit 3
///     wet,        // bit 4
///     pixel_body, // bit 5
/// });
///
/// let mut flags = PixelFlags::default();
/// assert!(!flags.burning());
/// flags.set_burning(true);
/// assert!(flags.burning());
/// assert_eq!(flags.bits(), 0b0000_1000);
/// ```
#[macro_export]
macro_rules! flags8 {
    ($name:ident { $($flag:ident),* $(,)? }) => {
        #[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
        #[repr(transparent)]
        pub struct $name(u8);

        impl $name {
            pub const EMPTY: Self = Self(0);

            #[inline]
            pub const fn new(bits: u8) -> Self {
                Self(bits)
            }

            #[inline]
            pub const fn bits(self) -> u8 {
                self.0
            }

            #[inline]
            pub fn from_bits(bits: u8) -> Self {
                Self(bits)
            }

            $crate::flags8!(@methods 0u8, $($flag),*);
        }

        impl ::core::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                let mut list = f.debug_set();
                $crate::flags8!(@debug self, list, 0u8, $($flag),*);
                list.finish()
            }
        }
    };

    // Internal: generate methods for each flag
    (@methods $bit:expr, $flag:ident $(, $rest:ident)*) => {
        ::paste::paste! {
            #[doc = concat!("Returns `true` if `", stringify!($flag), "` flag is set (bit ", stringify!($bit), ").")]
            #[inline]
            pub const fn $flag(self) -> bool {
                self.0 & (1 << $bit) != 0
            }

            #[doc = concat!("Sets or clears the `", stringify!($flag), "` flag (bit ", stringify!($bit), ").")]
            #[inline]
            pub fn [<set_ $flag>](&mut self, value: bool) {
                if value {
                    self.0 |= 1 << $bit;
                } else {
                    self.0 &= !(1 << $bit);
                }
            }
        }

        $crate::flags8!(@methods ($bit + 1u8), $($rest),*);
    };

    (@methods $bit:expr,) => {};

    // Internal: debug formatting
    (@debug $self:ident, $list:ident, $bit:expr, $flag:ident $(, $rest:ident)*) => {
        if $self.$flag() {
            $list.entry(&stringify!($flag));
        }
        $crate::flags8!(@debug $self, $list, ($bit + 1u8), $($rest),*);
    };

    (@debug $self:ident, $list:ident, $bit:expr,) => {};
}

/// Generate a nibbles struct packing two 4-bit values into one byte.
///
/// # Example
///
/// ```
/// # use pixel_macro::nibbles;
/// nibbles!(DamageVariant { damage, variant });
///
/// let mut dv = DamageVariant::new(5, 2);
/// assert_eq!(dv.damage(), 5);
/// assert_eq!(dv.variant(), 2);
///
/// dv.set_damage(15);
/// assert_eq!(dv.damage(), 15);
/// assert_eq!(dv.byte(), 0xF2); // damage in high nibble
/// ```
#[macro_export]
macro_rules! nibbles {
  ($name:ident { $high:ident, $low:ident }) => {
    #[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
    #[repr(transparent)]
    pub struct $name(u8);

    impl $name {
      #[inline]
      pub const fn new($high: u8, $low: u8) -> Self {
        debug_assert!($high < 16, concat!(stringify!($high), " must be < 16"));
        debug_assert!($low < 16, concat!(stringify!($low), " must be < 16"));
        Self(($high << 4) | ($low & 0x0F))
      }

      #[inline]
      pub const fn from_byte(byte: u8) -> Self {
        Self(byte)
      }

      #[inline]
      pub const fn byte(self) -> u8 {
        self.0
      }

      ::paste::paste! {
          #[doc = concat!("Returns the `", stringify!($high), "` value (high nibble, 0-15).")]
          #[inline]
          pub const fn $high(self) -> u8 {
              self.0 >> 4
          }

          #[doc = concat!("Sets the `", stringify!($high), "` value (high nibble, 0-15).")]
          #[inline]
          pub fn [<set_ $high>](&mut self, value: u8) {
              debug_assert!(value < 16, concat!(stringify!($high), " must be < 16"));
              self.0 = (self.0 & 0x0F) | (value << 4);
          }

          #[doc = concat!("Returns the `", stringify!($low), "` value (low nibble, 0-15).")]
          #[inline]
          pub const fn $low(self) -> u8 {
              self.0 & 0x0F
          }

          #[doc = concat!("Sets the `", stringify!($low), "` value (low nibble, 0-15).")]
          #[inline]
          pub fn [<set_ $low>](&mut self, value: u8) {
              debug_assert!(value < 16, concat!(stringify!($low), " must be < 16"));
              self.0 = (self.0 & 0xF0) | (value & 0x0F);
          }
      }
    }

    impl ::core::fmt::Debug for $name {
      fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.debug_struct(stringify!($name))
          .field(stringify!($high), &self.$high())
          .field(stringify!($low), &self.$low())
          .finish()
      }
    }
  };
}

/// Define a pixel struct with byte-aligned fields.
///
/// Creates a `#[repr(C)]` struct suitable for GPU upload. All fields are
/// byte-aligned for shader compatibility.
///
/// # Example
///
/// ```
/// # use pixel_macro::{define_pixel, flags8, nibbles};
/// // First define the component types
/// flags8!(PixelFlags { dirty, solid, falling, burning, wet, pixel_body });
/// nibbles!(DamageVariant { damage, variant });
///
/// // Then define the pixel struct
/// define_pixel!(Pixel {
///     material: u8,
///     color: u8,
///     damage_variant: DamageVariant,
///     flags: PixelFlags,
/// });
///
/// let mut pixel = Pixel::default();
/// pixel.set_material(5);
/// pixel.set_color(10);
/// pixel.damage_variant_mut().set_damage(3);
/// pixel.flags_mut().set_burning(true);
///
/// assert_eq!(pixel.material(), 5);
/// assert_eq!(pixel.color(), 10);
/// assert_eq!(pixel.damage_variant().damage(), 3);
/// assert!(pixel.flags().burning());
/// assert_eq!(core::mem::size_of::<Pixel>(), 4);
/// ```
#[macro_export]
macro_rules! define_pixel {
    ($name:ident { $($field:ident : $ty:ty),* $(,)? }) => {
        #[derive(Clone, Copy, Default, PartialEq, Eq)]
        #[repr(C)]
        pub struct $name {
            $($field: $ty,)*
        }

        impl $name {
            /// Creates a new pixel with all fields set to their default values.
            #[inline]
            pub fn new() -> Self {
                Self::default()
            }

            ::paste::paste! {
                $(
                    #[doc = concat!("Returns the `", stringify!($field), "` field.")]
                    #[inline]
                    pub const fn $field(&self) -> $ty {
                        self.$field
                    }

                    #[doc = concat!("Returns a mutable reference to the `", stringify!($field), "` field.")]
                    #[inline]
                    pub fn [<$field _mut>](&mut self) -> &mut $ty {
                        &mut self.$field
                    }

                    #[doc = concat!("Sets the `", stringify!($field), "` field.")]
                    #[inline]
                    pub fn [<set_ $field>](&mut self, value: $ty) {
                        self.$field = value;
                    }
                )*
            }

            /// Returns the pixel as a byte slice (for GPU upload).
            #[inline]
            pub fn as_bytes(&self) -> &[u8] {
                unsafe {
                    ::core::slice::from_raw_parts(
                        self as *const Self as *const u8,
                        ::core::mem::size_of::<Self>(),
                    )
                }
            }
        }

        impl ::core::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.debug_struct(stringify!($name))
                    $(.field(stringify!($field), &self.$field))*
                    .finish()
            }
        }
    };
}

#[cfg(test)]
mod tests {
  mod flags8_tests {
    crate::flags8!(TestFlags {
      dirty,
      solid,
      falling,
      burning,
      wet,
      pixel_body,
    });

    #[test]
    fn default_is_empty() {
      let flags = TestFlags::default();
      assert_eq!(flags.bits(), 0);
      assert!(!flags.dirty());
      assert!(!flags.solid());
      assert!(!flags.burning());
    }

    #[test]
    fn set_individual_flags() {
      let mut flags = TestFlags::default();

      flags.set_dirty(true);
      assert!(flags.dirty());
      assert_eq!(flags.bits(), 0b0000_0001);

      flags.set_burning(true);
      assert!(flags.burning());
      assert_eq!(flags.bits(), 0b0000_1001);

      flags.set_dirty(false);
      assert!(!flags.dirty());
      assert_eq!(flags.bits(), 0b0000_1000);
    }

    #[test]
    fn bit_positions_are_sequential() {
      let mut flags = TestFlags::default();

      flags.set_dirty(true); // bit 0
      assert_eq!(flags.bits(), 0b0000_0001);

      flags = TestFlags::default();
      flags.set_solid(true); // bit 1
      assert_eq!(flags.bits(), 0b0000_0010);

      flags = TestFlags::default();
      flags.set_falling(true); // bit 2
      assert_eq!(flags.bits(), 0b0000_0100);

      flags = TestFlags::default();
      flags.set_burning(true); // bit 3
      assert_eq!(flags.bits(), 0b0000_1000);

      flags = TestFlags::default();
      flags.set_wet(true); // bit 4
      assert_eq!(flags.bits(), 0b0001_0000);

      flags = TestFlags::default();
      flags.set_pixel_body(true); // bit 5
      assert_eq!(flags.bits(), 0b0010_0000);
    }

    #[test]
    fn from_bits_roundtrip() {
      let flags = TestFlags::from_bits(0b0010_1010);
      assert!(!flags.dirty()); // bit 0 = 0
      assert!(flags.solid()); // bit 1 = 1
      assert!(!flags.falling()); // bit 2 = 0
      assert!(flags.burning()); // bit 3 = 1
      assert!(!flags.wet()); // bit 4 = 0
      assert!(flags.pixel_body()); // bit 5 = 1
    }

    #[test]
    fn debug_format() {
      let mut flags = TestFlags::default();
      flags.set_burning(true);
      flags.set_wet(true);
      let debug = format!("{:?}", flags);
      assert!(debug.contains("burning"));
      assert!(debug.contains("wet"));
      assert!(!debug.contains("dirty"));
    }

    #[test]
    fn size_is_one_byte() {
      assert_eq!(core::mem::size_of::<TestFlags>(), 1);
    }
  }

  mod nibbles_tests {
    crate::nibbles!(DamageVariant { damage, variant });

    #[test]
    fn new_packs_correctly() {
      let dv = DamageVariant::new(5, 3);
      assert_eq!(dv.damage(), 5);
      assert_eq!(dv.variant(), 3);
      assert_eq!(dv.byte(), 0x53);
    }

    #[test]
    fn default_is_zero() {
      let dv = DamageVariant::default();
      assert_eq!(dv.damage(), 0);
      assert_eq!(dv.variant(), 0);
      assert_eq!(dv.byte(), 0);
    }

    #[test]
    fn set_high_nibble() {
      let mut dv = DamageVariant::new(0, 5);
      assert_eq!(dv.byte(), 0x05);

      dv.set_damage(15);
      assert_eq!(dv.damage(), 15);
      assert_eq!(dv.variant(), 5); // preserved
      assert_eq!(dv.byte(), 0xF5);
    }

    #[test]
    fn set_low_nibble() {
      let mut dv = DamageVariant::new(10, 0);
      assert_eq!(dv.byte(), 0xA0);

      dv.set_variant(7);
      assert_eq!(dv.damage(), 10); // preserved
      assert_eq!(dv.variant(), 7);
      assert_eq!(dv.byte(), 0xA7);
    }

    #[test]
    fn from_byte_roundtrip() {
      let dv = DamageVariant::from_byte(0xC9);
      assert_eq!(dv.damage(), 12);
      assert_eq!(dv.variant(), 9);
    }

    #[test]
    fn max_values() {
      let dv = DamageVariant::new(15, 15);
      assert_eq!(dv.damage(), 15);
      assert_eq!(dv.variant(), 15);
      assert_eq!(dv.byte(), 0xFF);
    }

    #[test]
    fn size_is_one_byte() {
      assert_eq!(core::mem::size_of::<DamageVariant>(), 1);
    }

    #[test]
    fn debug_format() {
      let dv = DamageVariant::new(7, 2);
      let debug = format!("{:?}", dv);
      assert!(debug.contains("damage: 7"));
      assert!(debug.contains("variant: 2"));
    }
  }

  mod define_pixel_tests {
    // Define component types
    crate::flags8!(TestFlags {
      dirty,
      solid,
      falling,
      burning,
      wet,
      pixel_body,
    });

    crate::nibbles!(DamageVariant { damage, variant });

    // Define the pixel struct
    crate::define_pixel!(TestPixel {
      material: u8,
      color: u8,
      damage_variant: DamageVariant,
      flags: TestFlags,
    });

    #[test]
    fn size_is_four_bytes() {
      assert_eq!(core::mem::size_of::<TestPixel>(), 4);
    }

    #[test]
    fn repr_c_layout() {
      // Verify fields are at expected byte offsets
      let pixel = TestPixel::default();
      let bytes = pixel.as_bytes();
      assert_eq!(bytes.len(), 4);
    }

    #[test]
    fn default_is_zeroed() {
      let pixel = TestPixel::default();
      assert_eq!(pixel.material(), 0);
      assert_eq!(pixel.color(), 0);
      assert_eq!(pixel.damage_variant().damage(), 0);
      assert_eq!(pixel.damage_variant().variant(), 0);
      assert!(!pixel.flags().dirty());
      assert!(!pixel.flags().burning());
    }

    #[test]
    fn set_material() {
      let mut pixel = TestPixel::default();
      pixel.set_material(42);
      assert_eq!(pixel.material(), 42);
    }

    #[test]
    fn set_color() {
      let mut pixel = TestPixel::default();
      pixel.set_color(128);
      assert_eq!(pixel.color(), 128);
    }

    #[test]
    fn set_damage_via_mut() {
      let mut pixel = TestPixel::default();
      pixel.damage_variant_mut().set_damage(10);
      assert_eq!(pixel.damage_variant().damage(), 10);
      assert_eq!(pixel.damage_variant().variant(), 0); // unchanged
    }

    #[test]
    fn set_flags_via_mut() {
      let mut pixel = TestPixel::default();
      pixel.flags_mut().set_dirty(true);
      pixel.flags_mut().set_burning(true);
      assert!(pixel.flags().dirty());
      assert!(pixel.flags().burning());
      assert!(!pixel.flags().wet());
    }

    #[test]
    fn as_bytes_matches_layout() {
      let mut pixel = TestPixel::default();
      pixel.set_material(0xAA);
      pixel.set_color(0xBB);
      pixel.damage_variant_mut().set_damage(0xC);
      pixel.damage_variant_mut().set_variant(0xD);
      pixel.flags_mut().set_dirty(true); // bit 0
      pixel.flags_mut().set_burning(true); // bit 3

      let bytes = pixel.as_bytes();
      assert_eq!(bytes[0], 0xAA); // material
      assert_eq!(bytes[1], 0xBB); // color
      assert_eq!(bytes[2], 0xCD); // damage (high) | variant (low)
      assert_eq!(bytes[3], 0b0000_1001); // dirty + burning
    }

    #[test]
    fn copy_semantics() {
      let mut pixel1 = TestPixel::default();
      pixel1.set_material(5);
      pixel1.flags_mut().set_burning(true);

      let pixel2 = pixel1; // copy
      assert_eq!(pixel2.material(), 5);
      assert!(pixel2.flags().burning());

      // Original unaffected by modifications to copy
      let mut pixel3 = pixel2;
      pixel3.set_material(99);
      assert_eq!(pixel1.material(), 5);
      assert_eq!(pixel2.material(), 5);
    }
  }
}
