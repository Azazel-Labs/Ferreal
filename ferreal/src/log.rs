//! Logging to Unreal's output log (the module's `LogFerreal<Module>` category), through the
//! table C++ registers when the module starts. Before that, messages go to stderr.

use crate::ffi::{RawObj, Str8, Table};

/// The module's table of C++ functions.
#[repr(C)]
pub struct ModuleTable {
    /// `level`: 0 info, 1 warning, 2 error.
    pub log: extern "C" fn(level: u8, message: Str8),
    /// The `UClass` of this name (`Actor`), or null.
    pub find_class: extern "C" fn(name: Str8) -> RawObj,
}

#[doc(hidden)]
pub static MODULE: Table<ModuleTable> = Table::new("the module");

static READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Keeps the module table and reports panics to Unreal's log (generated `ferreal_<module>_init`).
#[doc(hidden)]
pub fn init(table: *const ModuleTable) {
    MODULE.set(table);
    READY.store(true, std::sync::atomic::Ordering::Release);
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let at = info
            .location()
            .map(|l| format!(" at {}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .map(ToString::to_string)
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "a panic".into());
        send(2, &format!("panic{at}: {msg}"));
        default(info);
    }));
}

fn send(level: u8, message: &str) {
    if READY.load(std::sync::atomic::Ordering::Acquire) {
        (MODULE.get().log)(level, Str8::new(message));
    } else {
        eprintln!("ferreal: {message}");
    }
}

pub fn info(message: &str) {
    send(0, message);
}

pub fn warn(message: &str) {
    send(1, message);
}

pub fn error(message: &str) {
    send(2, message);
}

/// `ferreal::info!("...")`: formats and logs an informational message.
#[macro_export]
macro_rules! info {
    ($($t:tt)*) => { $crate::log::info(&format!($($t)*)) };
}

/// `ferreal::warn!("...")`: formats and logs a warning.
#[macro_export]
macro_rules! warn {
    ($($t:tt)*) => { $crate::log::warn(&format!($($t)*)) };
}

/// `ferreal::error!("...")`: formats and logs an error.
#[macro_export]
macro_rules! error {
    ($($t:tt)*) => { $crate::log::error(&format!($($t)*)) };
}
