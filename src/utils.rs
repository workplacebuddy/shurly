use std::env::var;

/// Get the value of ENV var, or a default
///
/// Only when:
/// - It is set
/// - It is not empty
pub fn env_var_or_else(var_name: &'static str, or_else: fn() -> String) -> String {
    if let Ok(value) = var(var_name) {
        if !value.is_empty() {
            return value;
        }
    }

    or_else()
}
