//! `takepkg rs.os;` - the machine and process a program runs in, for `std.os`.

use crate::value::*;

fn text(value: &Value, what: &str) -> RResult<String> {
    match value {
        Value::Str(text) => Ok(text.to_string()),
        other => type_err(format!("{what} expects a string, got {}", other.type_name())),
    }
}

fn path(path: Option<std::path::PathBuf>) -> Value {
    match path {
        Some(path) => Value::str(&path.to_string_lossy()),
        None => Value::Null,
    }
}

pub fn package() -> Value {
    NativePackage::new("rs.os")
        // One environment variable, or null when it is not set. A value that
        // is not valid Unicode is read with replacement characters.
        .function("env", 1, |args| {
            let name = text(&args[0], "os.env")?;
            if name.is_empty() || name.contains('=') || name.contains('\0') {
                return range_err(format!("os.env needs a variable name without '=' or NUL, got \"{name}\""));
            }
            Ok(std::env::var_os(&name).map_or(Value::Null, |value| Value::str(&value.to_string_lossy())))
        })
        // Every environment variable as a map, sorted by name so output is stable.
        // Windows ignores case in names (Path and PATH are one variable), so
        // there the names are upper-cased, as Python's os.environ does, and
        // environment().get("PATH") works on every platform.
        .function("environment", 0, |_| {
            let mut pairs: Vec<(String, String)> = std::env::vars_os()
                .map(|(k, v)| {
                    let name = k.to_string_lossy().into_owned();
                    let name = if cfg!(windows) { name.to_uppercase() } else { name };
                    (name, v.to_string_lossy().into_owned())
                })
                .collect();
            pairs.sort();
            let mut items = indexmap::IndexMap::new();
            for (name, value) in pairs {
                let key = Value::str(&name);
                items.insert(Key::of(&key), (key, Value::str(&value)));
            }
            Ok(Value::new_map(items, Meta::default()))
        })
        .function("cwd", 0, |_| match std::env::current_dir() {
            Ok(dir) => Ok(Value::str(&dir.to_string_lossy())),
            Err(e) => err(ErrorKind::Error, format!("the working directory cannot be read: {e}")),
        })
        .function("home", 0, |_| Ok(path(
            std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).filter(|h| !h.is_empty()).map(Into::into)
        )))
        // "windows", "linux", "macos", ...: Rust's names for the platforms.
        .function("platform", 0, |_| Ok(Value::str(std::env::consts::OS)))
        .function("arch", 0, |_| Ok(Value::str(std::env::consts::ARCH)))
        .function("separator", 0, |_| Ok(Value::str(std::path::MAIN_SEPARATOR_STR)))
        .function("pid", 0, |_| {
            let id = std::process::id() as i64;
            Ok(if i32::try_from(id).is_ok() { Value::Int(id) } else { Value::Long(id) })
        })
        .build()
}
