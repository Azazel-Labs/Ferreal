//! Unreal's value types, laid out as Unreal lays them out.

/// `FLinearColor`: linear RGBA, as floats.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LinearColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl LinearColor {
    pub const WHITE: LinearColor = LinearColor::new(1.0, 1.0, 1.0, 1.0);
    pub const BLACK: LinearColor = LinearColor::new(0.0, 0.0, 0.0, 1.0);
    pub const TRANSPARENT: LinearColor = LinearColor::new(0.0, 0.0, 0.0, 0.0);

    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        LinearColor { r, g, b, a }
    }

    pub fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// The color in sRGB (gamma encoded), as glyph colors are given to the shaders. Alpha
    /// stays linear.
    pub fn to_srgb(self) -> [f32; 4] {
        let enc = |c: f32| {
            let c = c.clamp(0.0, 1.0);
            if c <= 0.003_130_8 {
                c * 12.92
            } else {
                1.055 * c.powf(1.0 / 2.4) - 0.055
            }
        };
        [enc(self.r), enc(self.g), enc(self.b), self.a]
    }
}

/// `FVector2D`: two doubles.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vector2 {
    pub x: f64,
    pub y: f64,
}

impl Vector2 {
    pub const ZERO: Vector2 = Vector2::new(0.0, 0.0);

    pub const fn new(x: f64, y: f64) -> Self {
        Vector2 { x, y }
    }
}

/// `FVector`: three doubles.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vector3 {
    pub const ZERO: Vector3 = Vector3::new(0.0, 0.0, 0.0);

    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Vector3 { x, y, z }
    }
}

/// `FRotator`: pitch, yaw and roll in degrees, doubles.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rotator {
    pub pitch: f64,
    pub yaw: f64,
    pub roll: f64,
}

impl Rotator {
    pub const ZERO: Rotator = Rotator::new(0.0, 0.0, 0.0);

    pub const fn new(pitch: f64, yaw: f64, roll: f64) -> Self {
        Rotator { pitch, yaw, roll }
    }
}

/// A `#[uenum]`: converted to and from Unreal's `uint8` (the macro implements it).
pub trait UEnum: Copy + Default {
    fn to_u8(self) -> u8;
    /// The variant with this value, or the default for values the enum doesn't have.
    fn from_u8(v: u8) -> Self;
}
