//! The `#[repr(C)]` types the generated glue passes between Rust and C++, and the helpers
//! the generated Rust calls. The C++ side declares the same types in `FerrealRuntime.h`.
//!
//! Nothing here is meant to be used directly; the macros write the calls.

use std::cell::RefCell;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicPtr, Ordering};

/// A `UObject*` (or null).
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RawObj(pub *mut c_void);

impl RawObj {
    pub const NULL: RawObj = RawObj(std::ptr::null_mut());

    pub fn is_null(self) -> bool {
        self.0.is_null()
    }
}

impl Default for RawObj {
    fn default() -> Self {
        RawObj::NULL
    }
}

/// A view of UTF-16 text owned by C++ (an `FString`'s characters), valid during the call.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Str16 {
    pub ptr: *const u16,
    pub len: usize,
}

impl Str16 {
    /// The text, with unpaired surrogates replaced.
    pub fn to_string_lossy(self) -> String {
        if self.ptr.is_null() || self.len == 0 {
            return String::new();
        }
        // SAFETY: C++ passes `len` characters at `ptr`, alive for the call.
        let units = unsafe { std::slice::from_raw_parts(self.ptr, self.len) };
        String::from_utf16_lossy(units)
    }
}

impl Default for Str16 {
    fn default() -> Self {
        Str16 {
            ptr: std::ptr::null(),
            len: 0,
        }
    }
}

/// A view of UTF-8 text owned by Rust, which C++ copies before the call returns.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Str8 {
    pub ptr: *const u8,
    pub len: usize,
}

impl Str8 {
    /// A view of `s`, valid while `s` lives.
    pub fn new(s: &str) -> Str8 {
        Str8 {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }
}

impl Default for Str8 {
    fn default() -> Self {
        Str8 {
            ptr: std::ptr::null(),
            len: 0,
        }
    }
}

thread_local! {
    static RETURNED: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Returns a string to C++: kept in a per-thread buffer until the next string this thread
/// returns. C++ copies it at once.
pub fn return_str(s: String) -> Str8 {
    RETURNED.with_borrow_mut(|r| {
        *r = s;
        Str8::new(r)
    })
}

/// The number types arrays of which Rust returns to C++.
pub trait ReturnSlice: Sized + Copy + 'static {
    #[doc(hidden)]
    fn with_buffer<R>(f: impl FnOnce(&mut Vec<Self>) -> R) -> R;
}

macro_rules! return_slice {
    ($($t:ty),*) => {$(
        impl ReturnSlice for $t {
            fn with_buffer<R>(f: impl FnOnce(&mut Vec<Self>) -> R) -> R {
                thread_local! {
                    static BUFFER: RefCell<Vec<$t>> = const { RefCell::new(Vec::new()) };
                }
                BUFFER.with_borrow_mut(f)
            }
        }
    )*};
}

return_slice!(f32, u8, u32, i32);

/// Returns an array to C++: kept in a per-thread buffer (one per element type) until the
/// next array of that type this thread returns. C++ copies it at once.
pub fn return_slice<T: ReturnSlice>(v: Vec<T>) -> Slice<T> {
    T::with_buffer(|b| {
        *b = v;
        Slice::new(b)
    })
}

/// A view of an array, valid during the call.
#[repr(C)]
#[derive(Debug)]
pub struct Slice<T> {
    pub ptr: *const T,
    pub len: usize,
}

impl<T> Clone for Slice<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Slice<T> {}

impl<T> Default for Slice<T> {
    fn default() -> Self {
        Slice {
            ptr: std::ptr::null(),
            len: 0,
        }
    }
}

impl<T> Slice<T> {
    pub fn new(s: &[T]) -> Slice<T> {
        Slice {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }

    /// The elements.
    ///
    /// # Safety
    /// `ptr` and `len` describe live elements, as the generated glue passes them, for as
    /// long as the result is used.
    pub unsafe fn as_slice<'a>(self) -> &'a [T] {
        if self.ptr.is_null() || self.len == 0 {
            return &[];
        }
        // SAFETY: the caller's contract.
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
}

/// A table of C++ functions that C++ hands to Rust once (the generated registration).
pub struct Table<T> {
    ptr: AtomicPtr<T>,
    name: &'static str,
}

impl<T> Table<T> {
    pub const fn new(name: &'static str) -> Self {
        Table {
            ptr: AtomicPtr::new(std::ptr::null_mut()),
            name,
        }
    }

    /// Keeps `table`, which C++ owns for the rest of the program (a static).
    pub fn set(&self, table: *const T) {
        self.ptr.store(table.cast_mut(), Ordering::Release);
    }

    /// The table. Panics if C++ has not registered it: the generated `Register()` runs
    /// before any Rust is called.
    pub fn get(&self) -> &T {
        let p = self.ptr.load(Ordering::Acquire);
        assert!(
            !p.is_null(),
            "ferreal: the C++ table of {} is not registered",
            self.name
        );
        // SAFETY: C++ registered a static table, alive for the program.
        unsafe { &*p }
    }
}

/// Runs `f` for C++, catching a panic: it is logged and the default value returned, so Rust
/// never unwinds into C++.
pub fn guard<R: Default>(what: &str, f: impl FnOnce() -> R) -> R {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(payload) => {
            let msg = payload
                .downcast_ref::<&str>()
                .map(ToString::to_string)
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "a panic".into());
            crate::log::error(&format!(
                "{what}: Rust panicked ({msg}); C++ gets the default value"
            ));
            R::default()
        }
    }
}

/// The Rust state of an object: what `<Class>_new` returns to C++ and every later call
/// receives.
pub struct State<T> {
    cell: RefCell<T>,
    class: &'static str,
}

impl<T> State<T> {
    pub fn into_raw(value: T, class: &'static str) -> *mut c_void {
        Box::into_raw(Box::new(State {
            cell: RefCell::new(value),
            class,
        }))
        .cast()
    }

    /// Drops the state behind `ptr`.
    ///
    /// # Safety
    /// `ptr` came from [`State::into_raw`] for this `T`, and is not used again.
    pub unsafe fn drop_raw(ptr: *mut c_void) {
        if !ptr.is_null() {
            // SAFETY: the caller's contract.
            drop(unsafe { Box::from_raw(ptr.cast::<State<T>>()) });
        }
    }

    /// Runs `f` with the state behind `ptr`, borrowed mutably. A call while the state is
    /// borrowed (C++ calling back into the same object) is refused and logged.
    ///
    /// # Safety
    /// `ptr` came from [`State::into_raw`] for this `T` and has not been dropped.
    pub unsafe fn with_mut<R: Default>(
        ptr: *mut c_void,
        method: &str,
        f: impl FnOnce(&mut T) -> R,
    ) -> R {
        // SAFETY: the caller's contract.
        let state = unsafe { &*ptr.cast::<State<T>>() };
        match state.cell.try_borrow_mut() {
            Ok(mut v) => f(&mut v),
            Err(_) => {
                crate::log::error(&format!(
                    "{}::{method}: called again while the object is in use (C++ called back into it); skipped",
                    state.class
                ));
                R::default()
            }
        }
    }

    /// [`State::with_mut`], borrowed shared: calls on `&self` methods may nest.
    ///
    /// # Safety
    /// As [`State::with_mut`].
    pub unsafe fn with_ref<R: Default>(
        ptr: *mut c_void,
        method: &str,
        f: impl FnOnce(&T) -> R,
    ) -> R {
        // SAFETY: the caller's contract.
        let state = unsafe { &*ptr.cast::<State<T>>() };
        match state.cell.try_borrow() {
            Ok(v) => f(&v),
            Err(_) => {
                crate::log::error(&format!(
                    "{}::{method}: called while the object is being changed (C++ called back into it); skipped",
                    state.class
                ));
                R::default()
            }
        }
    }
}

/// Phantom data that keeps a type `Send` and `Sync`-neutral for markers.
pub type Marker<T> = PhantomData<fn() -> T>;
