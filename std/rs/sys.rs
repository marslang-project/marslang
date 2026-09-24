//! `takepkg rs.sys;` - the interpreter itself, for `std.sys`.

use crate::value::*;

pub fn package() -> Value {
    NativePackage::new("rs.sys")
        .function("version", 0, |_| Ok(Value::str(crate::VERSION)))
        .function("executable", 0, |_| Ok(match std::env::current_exe() {
            Ok(path) => Value::str(&path.to_string_lossy()),
            Err(_) => Value::Null,
        }))
        .function("package_dir", 0, |_| Ok(match crate::package::user_packages() {
            Some(path) => Value::str(&path.to_string_lossy()),
            None => Value::Null,
        }))
        // Every bundled standard package, by the name takepkg uses.
        .function("standard_packages", 0, |_| {
            let mut names: Vec<&str> = crate::package::BUNDLED.iter().map(|(name, ..)| *name).collect();
            names.sort_by_key(|name| name.to_lowercase());
            Ok(Value::array(names.into_iter().map(Value::str).collect()))
        })
        .build()
}
