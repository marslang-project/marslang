//! `takepkg`: loading packages. The model is similar to Python's packages.
//!
//! - `takepkg std.NAME;` loads a standard package bundled into the executable.
//!   Standard packages are Marslang source under `std/`.
//! - `takepkg rs.NAME;` loads a native package written in Rust (`std/rs/NAME.rs`),
//!   compiled into the interpreter. Only standard packages may import them.
//! - `takepkg a.b;` is an absolute import from the program's root directory (the
//!   main file's directory): `a/b/init.mars` if `a/b` is a package directory,
//!   otherwise `a/b.mars`. Parent packages' `init.mars` files run first. A package
//!   the program does not carry is looked up next in the user's package directory,
//!   `$MARSLANG_PKGS` or `marslang_pkgs` in the home directory, where the installer
//!   puts packages that every program of this user may import.
//! - `takepkg .b;` / `takepkg ..c.d;` are relative imports: one dot is the current
//!   package, each further dot goes up one level. The main program and top-level
//!   files are not in a package, so they cannot use relative imports.
//!
//! `build.rs` registers every `std/*.mars` and `std/rs/*.rs` file.
//!
//! The alias defaults to the last name segment (`takepkg std.math;` binds `math`).
//! Each package is loaded once, under its absolute name, and shared by every
//! import. A package exports its functions, families, and `fixed`/`hot` top-level
//! bindings, except names that start with `_`. Its top-level statements run once,
//! before the importer's.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::ast::*;

include!(concat!(env!("OUT_DIR"), "/std_packages.rs"));

pub(crate) enum Source { Native(fn() -> crate::value::Value), Program(Program) }

pub(crate) enum Export {
    /// A top-level binding, by its resolved global name.
    Binding(String),
    Function(String),
    Family(String),
}

pub(crate) struct LoadedPackage {
    /// Absolute dotted name, which also identifies the package.
    pub key: String,
    pub name: String,
    pub source: Source,
    pub exports: Vec<(String, Export)>,
}

pub(crate) struct Loader {
    /// Directories that absolute package names are resolved against, in order:
    /// the program's own root, then the user's package directory.
    roots: Vec<PathBuf>,
    /// Loaded packages, each after the packages it imports.
    pub packages: Vec<LoadedPackage>,
    loaded: HashSet<String>,
    /// Names of packages being loaded, to report import cycles.
    loading: Vec<String>,
}

/// Where a package's source comes from.
enum Found {
    Native(fn() -> crate::value::Value),
    Bundled(&'static str),
    /// A `.mars` file, the root it was found under, and `true` when it is a
    /// directory's `init.mars`.
    File(PathBuf, PathBuf, bool),
}

impl Loader {
    pub fn new(root: &Path) -> Self {
        let mut roots = vec![root.to_path_buf()];
        match user_packages() {
            Some(user) if user != roots[0] => roots.push(user),
            _ => {}
        }
        Loader { roots, packages: Vec::new(), loaded: HashSet::new(), loading: Vec::new() }
    }

    /// Load every package `program` imports, recursively, and record each
    /// import's package key and alias. `package` is the importer's enclosing
    /// package (`None` for the main program), which relative imports start from.
    /// Repeated identical imports are dropped.
    pub fn load_imports(&mut self, program: &mut Program, package: Option<&str>) -> Result<(), String> {
        let standard = package.is_some_and(|p| p == "std" || p.starts_with("std."));
        let mut seen = HashSet::new();
        let mut keep = Vec::with_capacity(program.items.len());
        for item in program.items.iter_mut() {
            let Item::Import(import) = item else { keep.push(true); continue };
            let line = import.line;
            let kept = self.load_import(import, package, standard, &mut seen)
                .map_err(|e| crate::parser::at_line(line, e))?;
            keep.push(kept);
        }
        let mut keep = keep.into_iter();
        program.items.retain(|_| keep.next().unwrap());
        Ok(())
    }

    /// Load what one `takepkg` names, recording its package key and alias.
    /// Returns false when the same package and alias were already imported here.
    fn load_import(&mut self, import: &mut ImportDecl, package: Option<&str>, standard: bool,
                   seen: &mut HashSet<(String, String)>) -> Result<bool, String> {
        let written = import.module.trim().to_string();
        if import.alias.as_deref() == Some("*") { return Err("wildcard imports are not implemented yet".into()); }
        let module = resolve_name(&written, package)?;
        if module.split('.').any(|segment| !is_ident(segment)) {
            return Err(format!("invalid package name '{written}'"));
        }
        if module.split('.').any(|segment| segment == "init") {
            return Err(format!("'init' is reserved for package init files; import the package itself instead of '{written}'"));
        }
        let alias = import.alias.clone().unwrap_or_else(|| module.rsplit('.').next().unwrap().to_string());
        if !is_ident(&alias) { return Err(format!("invalid import alias '{alias}' for package '{written}'")); }
        if (module == "rs" || module.starts_with("rs.")) && !standard {
            return Err(format!("native package '{module}' is only available to standard packages"));
        }
        self.load(&module)?;
        let fresh = seen.insert((module.clone(), alias.clone()));
        import.alias = Some(alias);
        import.key = Some(module);
        Ok(fresh)
    }

    /// Load a package by absolute name, after its parent packages.
    fn load(&mut self, module: &str) -> Result<(), String> {
        if self.loaded.contains(module) { return Ok(()); }
        if let Some(start) = self.loading.iter().position(|name| name == module) {
            let chain: Vec<&str> = self.loading[start..].iter().map(String::as_str).chain([module]).collect();
            return Err(format!("circular package import: {}", chain.join(" -> ")));
        }
        let found = self.find(module)?;
        if let Found::File(_, root, _) = &found {
            // Importing a.b.c first runs a/init.mars and a/b/init.mars, from the
            // same root as the package itself. A parent that is already loading
            // (its init imports this child) is skipped.
            let root = root.clone();
            let segments: Vec<&str> = module.split('.').collect();
            for end in 1..segments.len() {
                let parent = segments[..end].join(".");
                if !self.loading.contains(&parent) && init_file(&root, &parent).is_file() { self.load(&parent)?; }
            }
            if self.loaded.contains(module) { return Ok(()); }
        }
        let (source, exports) = match found {
            Found::Native(native) => (Source::Native(native), Vec::new()),
            Found::Bundled(text) => self.compile_package(module, text, false)?,
            Found::File(path, _, is_init) => {
                let text = fs::read_to_string(&path)
                    .map_err(|e| format!("failed to read package '{module}' at {}: {e}", path.display()))?;
                self.compile_package(module, &text, is_init)?
            }
        };
        self.packages.push(LoadedPackage { key: module.to_string(), name: module.to_string(), source, exports });
        self.loaded.insert(module.to_string());
        Ok(())
    }

    /// Marker names exported by `std.Decorator`, when it is loaded.
    pub fn markers(&self) -> Vec<String> {
        self.packages.iter().find(|package| package.key == "std.Decorator")
            .map(|package| package.exports.iter().map(|(name, _)| name.clone()).collect())
            .unwrap_or_default()
    }

    fn find(&self, module: &str) -> Result<Found, String> {
        if module == "rs" || module.starts_with("rs.") {
            return native::PACKAGES.iter().find(|(name, _)| *name == module).map(|(_, p)| Found::Native(*p))
                .ok_or_else(|| format!("native package '{module}' does not exist"));
        }
        if module == "std" || module.starts_with("std.") {
            return BUNDLED.iter().find(|(name, _)| *name == module).map(|(_, text)| Found::Bundled(text))
                .ok_or_else(|| format!("standard package '{module}' is not implemented yet"));
        }
        let mut tried = Vec::new();
        for root in &self.roots {
            // A package directory takes precedence over a module file of the same name.
            let init = init_file(root, module);
            if init.is_file() { return Ok(Found::File(init, root.clone(), true)); }
            let mut file = root.clone();
            file.extend(module.split('.'));
            file.set_extension("mars");
            if file.is_file() { return Ok(Found::File(file, root.clone(), false)); }
            tried.push(format!("{} and {}", init.display(), file.display()));
        }
        Err(format!("package '{module}' not found: looked for {}", tried.join(", ")))
    }

    fn compile_package(&mut self, module: &str, text: &str, is_init: bool) -> Result<(Source, Vec<(String, Export)>), String> {
        self.loading.push(module.to_string());
        let compiled = self.compile_source(module, text, is_init);
        self.loading.pop();
        let (program, exports) = compiled.map_err(|e| format!("in package {module}: {e}"))?;
        Ok((Source::Program(program), exports))
    }

    fn compile_source(&mut self, module: &str, text: &str, is_init: bool) -> Result<(Program, Vec<(String, Export)>), String> {
        let mut program = crate::parser::parse_program(text)?;
        // The enclosing package: a directory's init is its own package; a
        // module file belongs to its parent (none for a top-level file).
        let package = if is_init { Some(module) } else { module.rsplit_once('.').map(|(parent, _)| parent) };
        self.load_imports(&mut program, package)?;
        apply_decorators(&mut program, &self.markers())?;
        let constants: Vec<(usize, String)> = program.items.iter().enumerate().filter_map(|(index, item)| match item {
            Item::Var(v) if v.is_fixed || v.temp == TempKind::Hot => Some((index, v.name.clone())),
            _ => None,
        }).collect();
        crate::resolve::resolve(&mut program)?;
        let mut exports = Vec::new();
        for (index, name) in constants {
            let Item::Var(v) = &program.items[index] else { unreachable!("explicit declarations stay declarations") };
            exports.push((name, Export::Binding(v.name.clone())));
        }
        for item in &program.items {
            match item {
                // @Decorator.private, like a leading underscore, keeps a
                // declaration inside its own package.
                Item::Func(f) if f.access == Access::Private => {}
                Item::Family(f) if f.private => {}
                Item::Func(f) => exports.push((f.name.clone(), Export::Function(f.name.clone()))),
                Item::Family(f) => exports.push((f.name.clone(), Export::Family(f.name.clone()))),
                _ => {}
            }
        }
        exports.retain(|(name, _)| !name.starts_with('_'));
        Ok((program, exports))
    }
}

fn init_file(root: &Path, module: &str) -> PathBuf {
    let mut path = root.to_path_buf();
    path.extend(module.split('.'));
    path.join("init.mars")
}

/// The user's package directory: `$MARSLANG_PKGS`, else `marslang_pkgs` in the
/// home directory. Packages installed there are importable from every program.
/// The directory does not have to exist; it is reported when an import fails.
pub fn user_packages() -> Option<PathBuf> {
    match std::env::var_os("MARSLANG_PKGS") {
        Some(path) if !path.is_empty() => return Some(PathBuf::from(path)),
        _ => {}
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    Some(PathBuf::from(home).join("marslang_pkgs")).filter(|path| path.parent().is_some_and(|p| !p.as_os_str().is_empty()))
}

/// Turn a relative name (`.b`, `..c.d`) into an absolute one, from the
/// importer's enclosing package. Absolute names are returned unchanged.
fn resolve_name(written: &str, package: Option<&str>) -> Result<String, String> {
    let level = written.chars().take_while(|c| *c == '.').count();
    if level == 0 { return Ok(written.to_string()); }
    let rest = &written[level..];
    if rest.is_empty() { return Err(format!("relative import '{written}' must name a package, such as takepkg .name;")); }
    let package = package.filter(|p| !p.is_empty()).ok_or_else(|| {
        format!("relative import '{written}' has no parent package; the main program and top-level files must use absolute names")
    })?;
    let parts: Vec<&str> = package.split('.').collect();
    if level > parts.len() {
        return Err(format!("relative import '{written}' goes beyond the top-level package '{}'", parts[0]));
    }
    Ok(format!("{}.{rest}", parts[..parts.len() + 1 - level].join(".")))
}

fn is_ident(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Check the decorators on every function, family, and method, and record their
/// effect. A decorator `@X.name` needs `X` to be the alias of a
/// `takepkg std.Decorator;` in the file, and `name` one of its markers.
pub(crate) fn apply_decorators(program: &mut Program, markers: &[String]) -> Result<(), String> {
    let aliases: HashSet<String> = program.items.iter().filter_map(|item| match item {
        Item::Import(import) if import.key.as_deref() == Some("std.Decorator") => import.alias.clone(),
        _ => None,
    }).collect();
    let check = Decorators { aliases: &aliases, markers };
    for item in &mut program.items {
        match item {
            Item::Func(function) => {
                let line = function.line;
                let (doc, private) = check.docstring(&function.decorators, &format!("func {}", function.name), false)
                    .map_err(|e| crate::parser::at_line(line, e))?;
                function.doc = doc;
                if private { function.access = Access::Private; }
            }
            Item::Family(family) => {
                let line = family.line;
                let (doc, private) = check.docstring(&family.decorators, &format!("family {}", family.name), false)
                    .map_err(|e| crate::parser::at_line(line, e))?;
                family.doc = doc;
                family.private = private;
                for method in &mut family.methods {
                    let line = method.line;
                    check.method(&family.name, method).map_err(|e| crate::parser::at_line(line, e))?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

struct Decorators<'a> {
    aliases: &'a HashSet<String>,
    markers: &'a [String],
}

impl Decorators<'_> {
    /// The marker a decorator names, once its alias and name are known to be valid.
    fn marker<'d>(&self, decorator: &'d Decorator) -> Result<&'d str, String> {
        let written = &decorator.name;
        let (alias, name) = written.split_once('.').expect("the parser checks decorator shape");
        if !self.aliases.contains(alias) {
            return Err(format!("@{written} needs `takepkg std.Decorator;` (or an alias named {alias})"));
        }
        if !self.markers.iter().any(|marker| marker == name) {
            return Err(format!("unknown decorator @{written}; std.Decorator provides: {}", self.markers.join(", ")));
        }
        Ok(name)
    }

    /// Apply the decorators of a function or family that is not a method:
    /// `@Decorator.docstring` and `@Decorator.private` apply there. Returns the
    /// docstring, if any, and whether the declaration is private to its package.
    fn docstring(&self, decorators: &[Decorator], what: &str, method: bool) -> Result<(Option<String>, bool), String> {
        let mut doc = None;
        let mut private = false;
        for decorator in decorators {
            let name = self.marker(decorator)?;
            match name {
                "docstring" => {
                    let Some(Expr::String(text)) = &decorator.arg else {
                        return Err(format!("@{} needs one string, such as @{}(\"Adds two numbers.\")", decorator.name, decorator.name));
                    };
                    if doc.is_some() { return Err(format!("{what} has more than one @{}", decorator.name)); }
                    doc = Some(clean_doc(text));
                }
                // Outside a family there is no caller to check, so private
                // means one thing: the package does not export it.
                "private" if !method => {
                    if decorator.arg.is_some() { return Err(format!("@{} takes no argument", decorator.name)); }
                    private = true;
                }
                "subclass" if !method => {
                    return Err(format!("@{} applies to family methods, not to {what}", decorator.name));
                }
                "private" | "subclass" => {}
                _ => return Err(format!("@{} is not implemented yet", decorator.name)),
            }
        }
        Ok((doc, private))
    }

    fn method(&self, family: &str, method: &mut FuncDecl) -> Result<(), String> {
        method.doc = self.docstring(&method.decorators, &format!("{family}.{}", method.name), true)?.0;
        for decorator in &method.decorators {
            let access = match self.marker(decorator)? {
                "private" => Access::Private,
                "subclass" => Access::Subclass,
                _ => continue,
            };
            if decorator.arg.is_some() { return Err(format!("@{} takes no argument", decorator.name)); }
            if method.access != Access::Public && method.access != access {
                return Err(format!("{family}.{} cannot be both private and subclass", method.name));
            }
            if method.name == "init" {
                return Err(format!("{family}.init cannot be @{}; constructors are always public", decorator.name));
            }
            method.access = access;
        }
        Ok(())
    }
}

/// A docstring as written, without the indentation of the code around it: the
/// first line is trimmed, the common indentation of the rest is removed, and
/// blank lines at either end are dropped.
pub(crate) fn clean_doc(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let indent = lines.iter().skip(1)
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.chars().take_while(|c| c.is_whitespace()).count())
        .min()
        .unwrap_or(0);
    let cleaned: Vec<String> = lines.iter().enumerate().map(|(i, line)| {
        let line = if i == 0 { line.trim_start().to_string() } else { line.chars().skip(indent).collect() };
        line.trim_end().to_string()
    }).collect();
    let first = cleaned.iter().position(|line| !line.is_empty());
    let last = cleaned.iter().rposition(|line| !line.is_empty());
    match (first, last) {
        (Some(first), Some(last)) => cleaned[first..=last].join("\n"),
        _ => String::new(),
    }
}
