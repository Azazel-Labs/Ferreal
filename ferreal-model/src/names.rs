//! Naming conventions shared by both halves of the glue: C++ names of Rust items, and the
//! symbols of the `extern "C"` functions between them.

/// `font_size` → `FontSize`.
pub fn pascal(snake: &str) -> String {
    let mut out = String::with_capacity(snake.len());
    let mut upper = true;
    for c in snake.chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// A Rust identifier without a raw prefix (`r#type` → `type`).
pub fn unraw(ident: &str) -> &str {
    ident.strip_prefix("r#").unwrap_or(ident)
}

/// The symbols of a module's `extern "C"` functions. Every symbol starts with
/// `ferreal_<module>_`, so several Ferreal modules can be linked into one binary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Symbols {
    prefix: String,
}

impl Symbols {
    pub fn new(module: &str) -> Self {
        Symbols {
            prefix: format!("ferreal_{module}_"),
        }
    }

    /// Rust: receives the module's table of C++ functions (logging).
    pub fn init(&self) -> String {
        format!("{}init", self.prefix)
    }

    /// Rust: creates a class' Rust state for a new object.
    pub fn new_state(&self, class: &str) -> String {
        format!("{}{class}_new", self.prefix)
    }

    /// Rust: drops a class' Rust state.
    pub fn drop_state(&self, class: &str) -> String {
        format!("{}{class}_drop", self.prefix)
    }

    /// Rust: receives a class' table of property accessors.
    pub fn register_class(&self, class: &str) -> String {
        format!("{}{class}_register", self.prefix)
    }

    /// Rust: a method of a class that C++ calls (a UFUNCTION, an override, a hook).
    pub fn method(&self, class: &str, method: &str) -> String {
        format!("{}{class}_m_{}", self.prefix, unraw(method))
    }

    /// Rust: receives a C++ base's table of methods.
    pub fn register_base(&self, base: &str) -> String {
        format!("{}base_{base}_register", self.prefix)
    }

    /// Rust: receives the table of the engine bindings' C++ thunks (`ferreal::engine!`).
    pub fn register_engine(&self) -> String {
        format!("{}engine_register", self.prefix)
    }

    /// Rust: a free function exported to C++.
    pub fn export(&self, function: &str) -> String {
        format!("{}fn_{}", self.prefix, unraw(function))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(pascal("font_size"), "FontSize");
        assert_eq!(pascal("text"), "Text");
        assert_eq!(pascal("get_text_size"), "GetTextSize");
        assert_eq!(unraw("r#move"), "move");
        let s = Symbols::new("Glyphux");
        assert_eq!(
            s.method("GlyphuxText", "r#move"),
            "ferreal_Glyphux_GlyphuxText_m_move"
        );
        assert_eq!(
            s.register_base("WidgetBase"),
            "ferreal_Glyphux_base_WidgetBase_register"
        );
    }
}
