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

/// `FQuat`: a rotation as a unit quaternion, doubles.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quat {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

impl Quat {
    pub const IDENTITY: Quat = Quat::new(0.0, 0.0, 0.0, 1.0);

    pub const fn new(x: f64, y: f64, z: f64, w: f64) -> Self {
        Quat { x, y, z, w }
    }
}

impl Default for Quat {
    fn default() -> Self {
        Quat::IDENTITY
    }
}

/// `FTransform`: a rotation, a translation and a scale (copied through its accessors and
/// constructor: its C++ members are not public).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub rotation: Quat,
    pub translation: Vector3,
    pub scale: Vector3,
}

impl Transform {
    pub const IDENTITY: Transform = Transform {
        rotation: Quat::IDENTITY,
        translation: Vector3::ZERO,
        scale: Vector3::new(1.0, 1.0, 1.0),
    };
}

impl Default for Transform {
    fn default() -> Self {
        Transform::IDENTITY
    }
}

/// A `#[uenum]`: converted to and from Unreal's `uint8` (the macro implements it).
pub trait UEnum: Copy + Default {
    fn to_u8(self) -> u8;
    /// The variant with this value, or the default for values the enum doesn't have.
    fn from_u8(v: u8) -> Self;
}

/// A value that crosses to C++ through `#[repr(C)]` carriers: the engine structs and enums the
/// bindings generate (`ferreal::engine!`), and `#[uenum]`s. Generated code calls it.
pub trait Carry: Sized {
    /// What C++ hands Rust.
    type In: Copy;
    /// What Rust hands C++; it may point into `self` (strings), so it is used while `self`
    /// lives.
    type Out: Copy;
    fn from_in(c: Self::In) -> Self;
    fn to_out(&self) -> Self::Out;
}

/// The math types are their own carriers (`#[repr(C)]`, laid out as the C++ carriers are).
macro_rules! carry_self {
    ($($t:ty),*) => {$(
        impl Carry for $t {
            type In = $t;
            type Out = $t;
            fn from_in(c: $t) -> Self {
                c
            }
            fn to_out(&self) -> $t {
                *self
            }
        }
    )*};
}

carry_self!(
    LinearColor,
    Vector2,
    Vector3,
    Rotator,
    Quat,
    Transform,
    Vector4,
    IntPoint,
    IntVector,
    Color,
    Box3,
    Matrix,
    DateTime,
    Timespan
);

/// `FVector4`: four doubles.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vector4 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

impl Vector4 {
    pub const fn new(x: f64, y: f64, z: f64, w: f64) -> Self {
        Vector4 { x, y, z, w }
    }
}

/// `FIntPoint`: two `int32`s.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct IntPoint {
    pub x: i32,
    pub y: i32,
}

impl IntPoint {
    pub const fn new(x: i32, y: i32) -> Self {
        IntPoint { x, y }
    }
}

/// `FIntVector`: three `int32`s.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct IntVector {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl IntVector {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        IntVector { x, y, z }
    }
}

/// `FColor`: sRGB bytes (in RGBA order here, whatever order the platform keeps them in).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color { r, g, b, a }
    }
}

/// `FBox`: an axis-aligned box, valid once it holds a point.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Box3 {
    pub min: Vector3,
    pub max: Vector3,
    pub is_valid: bool,
}

impl Box3 {
    /// The box from `min` to `max` (valid).
    pub const fn new(min: Vector3, max: Vector3) -> Self {
        Box3 {
            min,
            max,
            is_valid: true,
        }
    }
}

/// `FMatrix`: four rows of four doubles (`M[row][column]`, as Unreal's).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix {
    pub m: [[f64; 4]; 4],
}

impl Matrix {
    pub const IDENTITY: Matrix = Matrix {
        m: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };
}

impl Default for Matrix {
    fn default() -> Self {
        Matrix::IDENTITY
    }
}

/// `FDateTime`: ticks of 100 nanoseconds since midnight, January 1, 0001.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DateTime {
    pub ticks: i64,
}

/// `FTimespan`: a duration, in ticks of 100 nanoseconds.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timespan {
    pub ticks: i64,
}

impl Timespan {
    pub const TICKS_PER_SECOND: i64 = 10_000_000;

    pub const fn from_seconds(seconds: i64) -> Self {
        Timespan {
            ticks: seconds * Self::TICKS_PER_SECOND,
        }
    }
}
