//! `takepkg rs.cli;` - the command line a program was started with, and
//! stopping it with an exit status. `std.cli` builds its parser on these.

use crate::value::*;

pub fn package() -> Value {
    NativePackage::new("rs.cli")
        // The words after the program's file: marslang tool.mars --name Ada
        .function("args", 0, |_| Ok(Value::array(program_args().iter().map(|a| Value::str(a)).collect())))
        .function("program", 0, |_| Ok(Value::str(&program_path())))
        // Stop now with this status. It passes every handle; then blocks still run.
        .function("exit", 1, |args| match &args[0] {
            Value::Int(code) if i32::try_from(*code).is_ok() => Err(exit_request(*code as i32)),
            other => type_err(format!("exit expects an int status, got {}", other.type_name())),
        })
        .build()
}
