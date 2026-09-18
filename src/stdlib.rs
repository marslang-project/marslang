use std::collections::HashSet;
use crate::ast::*;

// Bundled packages are resolved by the compiler, independently of the generated
// JavaScript's directory. General filesystem modules/exports remain separate work.
pub(crate) fn prepare(program: &mut Program) -> Result<String, String> {
    let mut math = false;
    let mut seen = HashSet::new();
    let mut duplicate = HashSet::new();
    for (index, item) in program.items.iter_mut().enumerate() {
        if let Item::Import(import) = item {
            if import.module == "std.math" {
                let alias = import.alias.get_or_insert_with(|| "math".into());
                if alias == "*" { return Err("wildcard imports are not implemented yet".into()); }
                let mut chars = alias.chars();
                if !chars.next().map(|c| c.is_ascii_alphabetic() || c == '_').unwrap_or(false)
                    || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    return Err("invalid std.math import alias".into());
                }
                if !seen.insert(alias.clone()) { duplicate.insert(index); }
                math = true;
            } else if import.module.starts_with("std.") {
                return Err(format!("standard package '{}' is not implemented yet", import.module));
            }
        }
    }
    let mut index = 0;
    program.items.retain(|_| { let keep = !duplicate.contains(&index); index += 1; keep });
    if !math { return Ok(String::new()); }
    let mut module = crate::parser::parse_program(include_str!("../std/math.mars"))
        .map_err(|e| format!("std.math: {e}"))?;
    crate::resolve::resolve(&mut module).map_err(|e| format!("std.math: {e}"))?;
    Ok(crate::eval::compile_math_module(&module))
}
