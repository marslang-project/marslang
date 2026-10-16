use crate::ast::*;

pub type PResult<T> = Result<T, String>;

/// A statement after normalization, with the source line it started on, so an
/// error can say where it is even though layout does not delimit statements.
pub struct Line {
    text: String,
    number: usize,
}

impl Line {
    fn trim(&self) -> &str {
        self.text.trim()
    }
}

/// Prefix a message with its source line, unless it already carries one.
pub(crate) fn at_line(number: usize, message: String) -> String {
    if message.starts_with("line ") { message } else { format!("line {number}: {message}") }
}

pub fn parse_program(input: &str) -> PResult<Program> {
    let lines = preprocess(input)?;
    let mut idx = 0usize;
    let mut items = Vec::new();
    while idx < lines.len() {
        let line = lines[idx].trim();
        if line.is_empty() || line == ";" {
            idx += 1;
            continue;
        }
        let number = lines[idx].number;
        if line.starts_with("takepkg ") {
            items.push(Item::Import(parse_import(line, number).map_err(|e| at_line(number, e))?));
            idx += 1;
            continue;
        }
        if line.starts_with("family ") {
            let (family, next) = parse_family(&lines, idx).map_err(|e| at_line(number, e))?;
            items.push(Item::Family(family));
            idx = next;
            continue;
        }
        if line.starts_with('@') {
            let (decorators, target, at) = collect_decorators(&lines, idx, line)?;
            let target_line = lines[at].number;
            if target.starts_with("func ") {
                let (mut func, next) = parse_func_from(&target, &lines, at).map_err(|e| at_line(target_line, e))?;
                func.decorators = decorators;
                items.push(Item::Func(func));
                idx = next;
            } else if target.starts_with("family ") {
                let (mut family, next) = parse_family_from(&target, &lines, at).map_err(|e| at_line(target_line, e))?;
                family.decorators = decorators;
                items.push(Item::Family(family));
                idx = next;
            } else {
                return Err(at_line(target_line, "decorators must be followed by a func or family".into()));
            }
            continue;
        }
        if line.starts_with("func ") {
            let (func, next) = parse_func(&lines, idx).map_err(|e| at_line(number, e))?;
            items.push(Item::Func(func));
            idx = next;
            continue;
        }
        let (stmt, next) = parse_stmt_at(&lines, idx)?;
        match stmt.kind {
            StmtKind::Var(mut v) => {
                v.line = stmt.line;
                items.push(Item::Var(v));
            }
            kind => items.push(Item::Stmt(Stmt { kind, line: stmt.line })),
        }
        idx = next;
    }

    for item in &items {
        match item {
            Item::Func(f) => validate_function_loops(f)?,
            Item::Family(f) => {
                for method in &f.methods { validate_function_loops(method)?; }
            }
            Item::Stmt(stmt) => validate_loop_control(std::slice::from_ref(stmt), 0)?,
            _ => {}
        }
    }
    Ok(Program { items })
}

fn validate_function_loops(function: &FuncDecl) -> PResult<()> {
    if let FuncBody::Block(body) = &function.body { validate_loop_control(body, 0)?; }
    Ok(())
}

fn validate_loop_control(body: &[Stmt], depth: usize) -> PResult<()> {
    for stmt in body {
        match &stmt.kind {
            StmtKind::Break | StmtKind::Continue if depth == 0 => {
                return Err("break/continue must be inside a loop".to_string());
            }
            StmtKind::Repeat { body, .. } | StmtKind::While { body, .. } |
            StmtKind::ForEach { body, .. } | StmtKind::For { body, .. } => validate_loop_control(body, depth + 1)?,
            StmtKind::If { then_block, elif_blocks, else_block, .. } => {
                validate_loop_control(then_block, depth)?;
                for (_, body) in elif_blocks { validate_loop_control(body, depth)?; }
                if let Some(body) = else_block { validate_loop_control(body, depth)?; }
            }
            StmtKind::Run { body, handlers, then_block } => {
                validate_loop_control(body, depth)?;
                for handler in handlers { validate_loop_control(&handler.body, depth)?; }
                if let Some(body) = then_block { validate_loop_control(body, depth)?; }
            }
            StmtKind::Match { arms, .. } => {
                for arm in arms { validate_loop_control(&arm.body, depth)?; }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Stands for a newline that was inside a triple-quoted string.
const LINE_MARK: char = '\u{E000}';

/// Whether the two characters after a `"` are also `"`.
fn opens_triple(chars: &std::iter::Peekable<std::str::Chars>) -> bool {
    let mut ahead = chars.clone();
    ahead.next() == Some('"') && ahead.next() == Some('"')
}

fn preprocess(input: &str) -> PResult<Vec<Line>> {
    let mut clean = String::new();
    let mut chars = input.chars().peekable();
    let mut quote = None;
    let mut escaped = false;
    let mut block_start = None;
    let mut line_comment = false;
    let (mut line, mut column) = (1usize, 0usize);
    let mut quote_start = (1usize, 0usize);
    while let Some(ch) = chars.next() {
        column += 1;
        if ch == '\n' {
            line += 1;
            column = 0;
            line_comment = false;
        }
        if block_start.is_some() {
            if ch == '*' && chars.peek() == Some(&'/') {
                chars.next();
                column += 1;
                block_start = None;
            } else if ch == '\n' {
                clean.push(ch);
            }
            continue;
        }
        if line_comment {
            continue;
        }
        if let Some(q) = quote {
            clean.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == q {
                quote = None;
            }
            continue;
        }
        if ch == '/' && chars.peek() == Some(&'/') {
            chars.next();
            column += 1;
            line_comment = true;
        } else if ch == '/' && chars.peek() == Some(&'*') {
            block_start = Some((line, column));
            chars.next();
            column += 1;
            clean.push(' ');
        } else if ch == '"' && opens_triple(&chars) {
            // """...""" spans lines. It becomes an ordinary one-line string, with raw
            // newlines and quotes escaped, followed by one LINE_MARK per newline so
            // later statements still know which source line they are on.
            let (start_line, start_column) = (line, column);
            chars.next();
            chars.next();
            column += 2;
            let mut newlines = 0;
            let mut escaped = false;
            clean.push('"');
            loop {
                let Some(c) = chars.next() else {
                    return Err(format!("line {start_line}: unterminated string starting at column {start_column}"));
                };
                column += 1;
                if c == '\n' {
                    line += 1;
                    column = 0;
                    newlines += 1;
                }
                if escaped {
                    escaped = false;
                    if c == '\n' { clean.push('n'); } else if c != '\r' { clean.push(c); }
                    continue;
                }
                match c {
                    '\\' => { escaped = true; clean.push('\\'); }
                    '"' if opens_triple(&chars) => {
                        chars.next();
                        chars.next();
                        column += 2;
                        break;
                    }
                    '"' => clean.push_str("\\\""),
                    '\n' => clean.push_str("\\n"),
                    '\r' => {}
                    _ => clean.push(c),
                }
            }
            clean.push('"');
            for _ in 0..newlines { clean.push(LINE_MARK); }
        } else if ch == 'r' && matches!(chars.peek(), Some('"' | '\''))
            && !clean.chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '_')
        {
            // r"...", r'...', and r"""...""" are raw: a backslash is only a
            // backslash, for regex patterns and Windows paths. Each becomes an
            // ordinary string with its backslashes and quotes escaped.
            let (start_line, start_column) = (line, column);
            let q = chars.next().unwrap();
            column += 1;
            let triple = q == '"' && opens_triple(&chars);
            if triple {
                chars.next();
                chars.next();
                column += 2;
            }
            let unterminated = || format!("line {start_line}: unterminated raw string starting at column {start_column}");
            let mut newlines = 0;
            clean.push('"');
            loop {
                let Some(c) = chars.next() else { return Err(unterminated()) };
                column += 1;
                if c == '\n' {
                    if !triple { return Err(unterminated()); }
                    line += 1;
                    column = 0;
                    newlines += 1;
                }
                if triple && c == '"' && opens_triple(&chars) {
                    chars.next();
                    chars.next();
                    column += 2;
                    break;
                }
                if !triple && c == q { break; }
                match c {
                    '\\' => clean.push_str("\\\\"),
                    '"' => clean.push_str("\\\""),
                    '\n' => clean.push_str("\\n"),
                    '\r' if triple => {}
                    _ => clean.push(c),
                }
            }
            clean.push('"');
            for _ in 0..newlines { clean.push(LINE_MARK); }
        } else {
            if ch == '\'' || ch == '"' {
                quote = Some(ch);
                quote_start = (line, column);
            }
            clean.push(ch);
        }
    }
    if let Some((line, column)) = block_start {
        return Err(format!("line {line}: unterminated block comment starting at column {column}"));
    }
    if quote.is_some() {
        let (line, column) = quote_start;
        return Err(format!("line {line}: unterminated string starting at column {column}"));
    }
    // Layout does not delimit statements. Split only outside strings and
    // parenthesized headers/calls, so compact blocks use the same parser path.
    // Each piece keeps the source line it started on, for error messages.
    let mut lines = Vec::new();
    let mut text = String::new();
    let mut number = 1usize;
    let mut source = 1usize;
    let mut quote = None;
    let mut escaped = false;
    let mut depth = 0usize;
    // Whether the statement being gathered went on past a line break.
    let mut continued = false;
    for ch in clean.chars() {
        if ch == LINE_MARK {
            source += 1;
            continue;
        }
        if ch == '\n' {
            source += 1;
            // Inside ( ) or [ ] the statement goes on, so a long call, an
            // array, or a parameter list can be laid out over several lines.
            if depth > 0 && quote.is_none() {
                text.push(' ');
                continued = true;
                continue;
            }
            lines.push(Line { text: std::mem::take(&mut text), number });
            continued = false;
            number = source;
            continue;
        }
        // No brace belongs inside ( ) or [ ], so one there, after a line
        // break, means a bracket was left open on an earlier line.
        if continued && depth > 0 && quote.is_none() && matches!(ch, '{' | '}') {
            return Err(format!("line {number}: a '(' or '[' on this line is never closed"));
        }
        if text.is_empty() { number = source; }
        text.push(ch);
        if let Some(q) = quote {
            if escaped { escaped = false; }
            else if ch == '\\' { escaped = true; }
            else if ch == q { quote = None; }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            '{' | '}' | ';' if depth == 0 => {
                lines.push(Line { text: std::mem::take(&mut text), number });
                continued = false;
            }
            _ => {}
        }
    }
    if !text.is_empty() { lines.push(Line { text, number }); }
    Ok(lines)
}

fn parse_import(line: &str, number: usize) -> PResult<ImportDecl> {
    let body = line
        .trim_end_matches(';')
        .trim_start_matches("takepkg")
        .trim();
    if let Some((module, alias)) = body.split_once('=') {
        Ok(ImportDecl {
            module: module.trim().to_string(),
            alias: Some(alias.trim().to_string()),
            key: None,
            line: number,
        })
    } else {
        Ok(ImportDecl {
            module: body.to_string(),
            alias: None,
            key: None,
            line: number,
        })
    }
}

fn parse_family(lines: &[Line], start: usize) -> PResult<(FamilyDecl, usize)> {
    parse_family_from(lines[start].trim(), lines, start)
}

/// Parse a family whose header is `header` (normally `lines[start]`).
fn parse_family_from(header: &str, lines: &[Line], start: usize) -> PResult<(FamilyDecl, usize)> {
    let mut header = header.trim_start_matches("family").trim().to_string();
    if !header.contains('{') {
        return Err("family must open with '{' on same line".to_string());
    }
    header = header[..header.find('{').unwrap_or(header.len())]
        .trim()
        .to_string();

    let (name, extends) = if let Some((n, rest)) = header.split_once('(') {
        (
            n.trim().to_string(),
            Some(rest.trim_end_matches(')').trim().to_string()),
        )
    } else {
        (header.trim().to_string(), None)
    };
    reserved(&name, "family")?;

    let mut methods = Vec::new();
    let mut i = start + 1;
    while i < lines.len() {
        let line = lines[i].trim();
        if line.starts_with('@') {
            let (decorators, target, at) = collect_decorators(lines, i, line)?;
            if !target.starts_with("func ") {
                return Err(at_line(lines[at].number, "decorators in a family must be followed by a func".into()));
            }
            let (mut func, next) = parse_func_from(&target, lines, at).map_err(|e| at_line(lines[at].number, e))?;
            func.decorators = decorators;
            methods.push(func);
            i = next;
            continue;
        }
        if line == "}" || line == "};" {
            return Ok((
                FamilyDecl {
                    name,
                    extends,
                    methods,
                    decorators: Vec::new(),
                    private: false,
                    doc: None,
                    line: lines[start].number,
                    end_line: lines[i].number,
                },
                i + 1,
            ));
        }
        if line.is_empty() || line == ";" {
            i += 1;
            continue;
        }
        if line.starts_with("func ") {
            let (func, next) = parse_func(lines, i).map_err(|e| at_line(lines[i].number, e))?;
            methods.push(func);
            i = next;
            continue;
        }
        return Err(format!("unexpected token in family body: '{line}'"));
    }
    Err("unterminated family block".to_string())
}

/// Read the decorators starting at `lines[idx]` (whose text is `first`), across
/// as many lines as they take. Returns them, the text of the declaration they
/// decorate, and the index of the line that text is on.
fn collect_decorators(lines: &[Line], idx: usize, first: &str) -> PResult<(Vec<Decorator>, String, usize)> {
    let mut decorators = Vec::new();
    let mut at = idx;
    let mut text = first.to_string();
    while text.starts_with('@') {
        let (more, rest) = parse_decorators(&text).map_err(|e| at_line(lines[at].number, e))?;
        decorators.extend(more);
        if !rest.is_empty() {
            text = rest;
            break;
        }
        at += 1;
        while at < lines.len() && matches!(lines[at].trim(), "" | ";") { at += 1; }
        if at >= lines.len() {
            return Err(at_line(lines[idx].number, "decorators must be followed by a func or family".into()));
        }
        text = lines[at].trim().to_string();
    }
    Ok((decorators, text, at))
}

/// `@Alias.name` and `@Alias.name(argument)` decorators at the start of `line`,
/// and the rest of the line after them.
fn parse_decorators(line: &str) -> PResult<(Vec<Decorator>, String)> {
    let mut decorators = Vec::new();
    let mut rest = line.trim_start();
    while let Some(after_at) = rest.strip_prefix('@') {
        let end = after_at.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.')).unwrap_or(after_at.len());
        let name = &after_at[..end];
        let parts: Vec<&str> = name.split('.').collect();
        if parts.len() != 2 || !parts.iter().all(|p| is_bare_ident(p)) {
            return Err(format!("invalid decorator '@{name}'; decorators look like @Decorator.private"));
        }
        let mut after = &after_at[end..];
        let mut arg = None;
        if after.starts_with('(') {
            let close = find_matching_paren(after).map_err(|_| format!("@{name} has an unclosed '('"))?;
            let inner = after[1..close].trim();
            if !inner.is_empty() { arg = Some(parse_expr(inner)?); }
            after = &after[close + 1..];
        }
        decorators.push(Decorator { name: name.to_string(), arg });
        rest = after.trim_start();
    }
    Ok((decorators, rest.to_string()))
}

/// The declaration of `func(params) => body`, an anonymous function.
pub(crate) fn lambda(params: &str, body: Expr) -> PResult<FuncDecl> {
    let (_, params) = parse_func_signature(&format!("anonymous({params})"))?;
    Ok(FuncDecl {
        name: "anonymous".into(),
        params,
        body: FuncBody::Expr(body),
        decorators: Vec::new(),
        access: Access::Public,
        doc: None,
        line: 0,
        end_line: 0,
    })
}

fn parse_func(lines: &[Line], start: usize) -> PResult<(FuncDecl, usize)> {
    parse_func_from(lines[start].trim(), lines, start)
}

/// Parse a function whose header is `header` (normally `lines[start]`); a block
/// body continues on the following lines.
fn parse_func_from(header: &str, lines: &[Line], start: usize) -> PResult<(FuncDecl, usize)> {
    if header.contains("=>") {
        let (left, right) = header
            .trim_end_matches(';')
            .split_once("=>")
            .ok_or_else(|| "invalid single-line function".to_string())?;
        let (name, params) = parse_func_signature(left.trim_start_matches("func").trim())?;
        // `func f(int x) => match (x){` takes the lines of the match with it.
        let (body, next) = match match_expression_start(right.trim()) {
            Some(at) => gather_match_expression(right.trim(), at, lines, start)?,
            None => (parse_expr(right.trim())?, start + 1),
        };
        return Ok((
            FuncDecl {
                name,
                params,
                body: FuncBody::Expr(body),
                decorators: Vec::new(),
                access: Access::Public,
                doc: None,
                line: lines[start].number,
                end_line: lines[next - 1].number,
            },
            next,
        ));
    }

    let sig = header
        .trim_start_matches("func")
        .trim()
        .trim_end_matches('{')
        .trim();
    let (name, params) = parse_func_signature(sig)?;

    let mut i = start + 1;
    let body = parse_block_stmts(lines, &mut i)?;
    Ok((
        FuncDecl {
            name,
            params,
            body: FuncBody::Block(body),
            decorators: Vec::new(),
            access: Access::Public,
            doc: None,
            line: lines[start].number,
            end_line: lines[i.saturating_sub(1).max(start)].number,
        },
        i,
    ))
}

fn parse_block_stmts(lines: &[Line], idx: &mut usize) -> PResult<Vec<Stmt>> {
    let mut stmts = Vec::new();
    while *idx < lines.len() {
        let line = lines[*idx].trim();
        if line.is_empty() || line == ";" {
            *idx += 1;
            continue;
        }
        if line == "}" || line == "};" {
            *idx += 1;
            return Ok(stmts);
        }

        let (stmt, next) = parse_stmt_at(lines, *idx)?;
        stmts.push(stmt);
        *idx = next;
    }
    Err("unterminated block".to_string())
}

fn parse_func_signature(sig: &str) -> PResult<(String, Vec<Param>)> {
    let function_name = sig.split('(').next().unwrap_or_default().trim();
    if !is_bare_ident(function_name) {
        return Err(format!("invalid function name '{function_name}'"));
    }
    reserved(function_name, "function")?;
    if let Some((name, rest)) = sig.split_once('(') {
        let params_part = rest.trim().strip_suffix(')')
            .ok_or_else(|| "function parameter list must end with ')'".to_string())?
            .trim();
        if params_part.contains(';') {
            return Err("function parameters use commas, not semicolons; omit the final semicolon".to_string());
        }
        let mut brackets = 0usize;
        for ch in params_part.chars() {
            match ch {
                '[' => brackets += 1,
                ']' if brackets > 0 => brackets -= 1,
                ']' => return Err("unmatched ']' in function parameters".to_string()),
                _ => {}
            }
        }
        if brackets != 0 {
            return Err("unclosed '[' in function parameters".to_string());
        }
        let mut params = Vec::new();
        if params_part.is_empty() {
            return Ok((name.trim().to_string(), params));
        }
        for p in split_top_level(params_part, ',') {
            let p = p.trim();
            let (ty, name) = p.rsplit_once(char::is_whitespace)
                .ok_or_else(|| format!("expected 'type name' parameter, got '{p}'"))?;
            let ty = ty.trim();
            let mut depth = 0usize;
            for ch in ty.chars() {
                match ch {
                    '[' => depth += 1,
                    ']' => depth -= 1,
                    ch if ch.is_whitespace() && depth == 0 => {
                        return Err(format!("expected comma between parameters in '{p}'"));
                    }
                    _ => {}
                }
            }
            if ty.is_empty() || !name.chars().enumerate().all(|(i, ch)| {
                ch == '_' || ch.is_ascii_alphabetic() || (i > 0 && ch.is_ascii_digit())
            }) || name.is_empty() {
                return Err(format!("invalid function parameter '{p}'"));
            }
            reserved(name, "parameter")?;
            params.push(Param {
                ty: ty.to_string(),
                name: name.to_string(),
            });
        }
        let mut names = std::collections::HashSet::new();
        for param in &params {
            if !names.insert(&param.name) {
                return Err(format!("duplicate parameter '{}'", param.name));
            }
        }
        Ok((name.trim().to_string(), params))
    } else {
        Ok((sig.trim().to_string(), Vec::new()))
    }
}

/// Whether `line` opens a block introduced by `keyword` alone, such as `run{`.
fn opens_keyword_block(line: &str, keyword: &str) -> bool {
    line.strip_prefix(keyword).and_then(|rest| rest.trim_start().strip_suffix('{')).is_some_and(|mid| mid.trim().is_empty())
}

fn parse_run_stmt(lines: &[Line], start: usize) -> PResult<(StmtKind, usize)> {
    let mut i = start + 1;
    let body = parse_block_stmts(lines, &mut i)?;
    let mut handlers = Vec::new();
    let mut then_block = None;
    while i < lines.len() {
        let l = lines[i].trim();
        if l.is_empty() { i += 1; continue; }
        if (l.starts_with("handle(") || l.starts_with("handle ")) && then_block.is_none() {
            if !l.ends_with('{') { return Err("handle header must end with '{'".into()); }
            let after = l.trim_start_matches("handle").trim().trim_end_matches('{').trim();
            if !after.starts_with('(') || find_matching_paren(after)? != after.len() - 1 {
                return Err("handle must list error types in parentheses, such as handle(Error e){".into());
            }
            let (types, name) = parse_handle_header(&after[1..after.len() - 1])?;
            i += 1;
            handlers.push(Handler { types, name, body: parse_block_stmts(lines, &mut i)? });
            continue;
        }
        if opens_keyword_block(l, "then") {
            i += 1;
            then_block = Some(parse_block_stmts(lines, &mut i)?);
            break;
        }
        break;
    }
    if handlers.is_empty() && then_block.is_none() {
        return Err("run needs at least one handle or then block".into());
    }
    Ok((StmtKind::Run { body, handlers, then_block }, i))
}

/// `handle(Error)`, `handle(TypeError, RangeError)` (no name), `handle(Error e)`,
/// or `handle([TypeError, RangeError] e)`.
fn parse_handle_header(header: &str) -> PResult<(Vec<String>, Option<String>)> {
    let header = header.trim();
    let type_name = |text: &str| -> PResult<String> {
        let text = text.trim();
        if text.split('.').all(is_bare_ident) { Ok(text.to_string()) }
        else { Err(format!("invalid error type '{text}' in handle")) }
    };
    // A trailing name after whitespace, outside brackets, binds the error.
    if let Some((ty, name)) = header.rsplit_once(char::is_whitespace) {
        let ty = ty.trim();
        if is_bare_ident(name) && !ty.is_empty() && split_top_level(ty, ',').len() == 1 {
            let types = match ty.strip_prefix('[').and_then(|t| t.strip_suffix(']')) {
                Some(list) => split_top_level(list, ',').iter().map(|t| type_name(t)).collect::<PResult<Vec<_>>>()?,
                None => vec![type_name(ty)?],
            };
            reserved(name, "handled error")?;
            return Ok((types, Some(name.to_string())));
        }
    }
    if header.is_empty() { return Err("handle needs at least one error type".into()); }
    if header.starts_with('[') {
        return Err("a bracketed error list needs a name, such as handle([TypeError, RangeError] e)".into());
    }
    let types = split_top_level(header, ',').iter().map(|t| type_name(t)).collect::<PResult<Vec<_>>>()?;
    Ok((types, None))
}

fn parse_stmt_at(lines: &[Line], idx: usize) -> PResult<(Stmt, usize)> {
    let number = lines[idx].number;
    let (kind, next) = parse_stmt_from(lines, idx).map_err(|e| at_line(number, e))?;
    Ok((Stmt { kind, line: number }, next))
}

fn parse_stmt_from(lines: &[Line], idx: usize) -> PResult<(StmtKind, usize)> {
    parse_stmt_text(lines[idx].trim(), lines, idx)
}

/// Parse the statement that starts at `lines[idx]`, whose text is `l`: the
/// line itself, or, for a match arm, what follows its `=>`.
fn parse_stmt_text(l: &str, lines: &[Line], idx: usize) -> PResult<(StmtKind, usize)> {
    if starts_with_word(l, "match") {
        if !l.ends_with('{') {
            return Err("'match' is a keyword: a match is written match (value){ pattern => ...; }".into());
        }
        return parse_match_stmt(l, lines, idx);
    }

    // A func inside a block: a closure over the variables around it.
    if l.starts_with("func ") {
        let (decl, next) = parse_func_from(l, lines, idx)?;
        validate_function_loops(&decl)?;
        return Ok((StmtKind::Func { name: decl.name.clone(), decl: std::sync::Arc::new(decl) }, next));
    }

    if opens_keyword_block(l, "run") {
        return parse_run_stmt(lines, idx);
    }

    if l.starts_with("while ") || l.starts_with("while(") {
        if !l.ends_with('{') { return Err("while must open a block".into()); }
        let cond = parse_condition_between_parens(l, "while")?;
        let mut next = idx + 1;
        let body = parse_block_stmts(lines, &mut next)?;
        return Ok((StmtKind::While { cond, body }, next));
    }
    if l.starts_with("for ") || l.starts_with("for(") {
        let header = l.trim_start_matches("for").trim().strip_suffix('{')
            .ok_or("for must open a block")?.trim();
        let header = header.strip_prefix('(').and_then(|s| s.strip_suffix(')'))
            .ok_or("for requires a parenthesized header")?;
        let parts = split_top_level(header, ',');
        if parts.len() == 3 {
            let init = parse_for_sequence(parts[0].trim(), lines[idx].number)?;
            let cond = parse_expr(parts[1].trim())?;
            let step = parse_for_sequence(parts[2].trim(), lines[idx].number)?;
            let mut next = idx + 1;
            let body = parse_block_stmts(lines, &mut next)?;
            return Ok((StmtKind::For { init, cond, step, body }, next));
        }
        if parts.len() != 2 {
            return Err("expected for (item, iterable) or for (initializer, condition, change)".into());
        }
        let name = parts[0].trim();
        if !is_bare_ident(name) || parts[1].trim().is_empty() {
            return Err("expected for (itemname, iterable)".into());
        }
        reserved(name, "loop variable")?;
        let iterable = parse_expr(parts[1].trim())?;
        let mut next = idx + 1;
        let body = parse_block_stmts(lines, &mut next)?;
        return Ok((StmtKind::ForEach { name: name.into(), iterable, body }, next));
    }

    if l.starts_with("if ") || l.starts_with("if(") {
        return parse_if_stmt(l, lines, idx);
    }
    if l.starts_with("repeat ") {
        return parse_repeat_stmt(l, lines, idx);
    }
    // `label = match (x){`: a match expression over the lines that follow.
    if let Some(at) = match_expression_start(l) {
        return parse_match_expression_stmt(l, at, lines, idx);
    }
    Ok((parse_stmt_line(l)?, idx + 1))
}

// ----- match -----

/// Stands in for a match expression written over several lines while the
/// statement around it is parsed; the parsed match then replaces it.
const MATCH_PLACEHOLDER: &str = "__match_expression__";

/// Whether `text` starts with the word `word`, not merely with those letters.
fn starts_with_word(text: &str, word: &str) -> bool {
    text.strip_prefix(word).is_some_and(|rest| !rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_'))
}

/// `match` cannot name anything a program declares.
fn reserved(name: &str, what: &str) -> PResult<()> {
    if name == "match" { return Err(format!("'match' is a keyword and cannot name a {what}")); }
    Ok(())
}

/// Where the match expression starts in a line that ends with its header,
/// such as `label = match (x){`; `None` for any other line.
fn match_expression_start(text: &str) -> Option<usize> {
    let header = text.strip_suffix('{')?.trim_end();
    if !header.ends_with(')') { return None; }
    // The '(' that the final ')' closes.
    let (mut opens, mut quote, mut escaped, mut open) = (Vec::new(), None, false, None);
    for (i, ch) in header.char_indices() {
        if let Some(q) = quote {
            if escaped { escaped = false; } else if ch == '\\' { escaped = true; } else if ch == q { quote = None; }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' => opens.push(i),
            ')' => open = opens.pop(),
            _ => {}
        }
    }
    let before = header[..open?].trim_end();
    let at = before.strip_suffix("match")?.len();
    let boundary = !before[..at].ends_with(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '.');
    boundary.then_some(at)
}

/// `match (subject){` (or `match(subject) {`): the subject.
fn match_subject(header: &str) -> PResult<Expr> {
    let usage = "a match is written match (value){ pattern => ...; }";
    let after = header.trim().strip_prefix("match").ok_or(usage)?.trim_start();
    if !after.starts_with('(') { return Err(usage.into()); }
    let close = find_matching_paren(after)?;
    if after[close + 1..].trim() != "{" { return Err(usage.into()); }
    let inner = after[1..close].trim();
    if inner.is_empty() { return Err("match needs the value to match between its parentheses".into()); }
    parse_expr(inner)
}

/// The first `=>` outside strings and brackets, splitting an arm into its
/// pattern (with any guard) and its body.
fn split_arm(text: &str) -> Option<(&str, &str)> {
    let (mut depth, mut quote, mut escaped) = (0usize, None, false);
    let mut chars = text.char_indices().peekable();
    while let Some((i, ch)) = chars.next() {
        if let Some(q) = quote {
            if escaped { escaped = false; } else if ch == '\\' { escaped = true; } else if ch == q { quote = None; }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            '=' if depth == 0 && matches!(chars.peek(), Some((_, '>'))) => return Some((&text[..i], &text[i + 2..])),
            _ => {}
        }
    }
    None
}

const NO_ARROW: &str = "a match arm needs '=>' between its pattern and its body, as in 1 => out(\"one\");";

/// Where the word `word` stands in `text`, outside strings and brackets.
fn find_word(text: &str, word: &str) -> Option<usize> {
    let (mut depth, mut quote, mut escaped) = (0usize, None, false);
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    for (i, ch) in text.char_indices() {
        if let Some(q) = quote {
            if escaped { escaped = false; } else if ch == '\\' { escaped = true; } else if ch == q { quote = None; }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 && text[i..].starts_with(word)
                && !text[..i].ends_with(ident) && !text[i + word.len()..].starts_with(ident) => return Some(i),
            _ => {}
        }
    }
    None
}

/// The pattern and guard of a match arm, from the text before its `=>`.
fn parse_arm_head(head: &str) -> PResult<(Pattern, Option<Expr>)> {
    let head = head.trim();
    let (pattern, guard) = match find_word(head, "if") {
        Some(at) => {
            let condition = head[at + 2..].trim();
            if !condition.starts_with('(') || find_matching_paren(condition)? != condition.len() - 1 {
                return Err("a match guard is written if (condition), with the condition in parentheses".into());
            }
            (head[..at].trim(), Some(parse_expr(&condition[1..condition.len() - 1])?))
        }
        None => (head, None),
    };
    Ok((parse_pattern(pattern)?, guard))
}

fn parse_pattern(text: &str) -> PResult<Pattern> {
    if text.is_empty() { return Err("a match arm needs a pattern before '=>'".into()); }
    if text == "_" { return Ok(Pattern::Wildcard); }
    let first_word = text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).next().unwrap_or("");
    let literal = !text.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_' || c == '[')
        || matches!(first_word, "true" | "false" | "fasle" | "null")
        || text.starts_with("f\"") || text.starts_with("f'");
    if literal {
        let mut literals = Vec::new();
        for part in split_top_level(text, ',') {
            let part = part.trim();
            if part.is_empty() {
                return Err("a list of literals in a match pattern has an empty item; separate the literals with single commas".into());
            }
            let not_literal = || format!("'{part}' is not a literal; a match pattern is literals such as 1 or \"quit\", _, or a type and a name such as int n");
            let value = parse_expr(part).map_err(|_| not_literal())?;
            let plain = match &value {
                Expr::Number(_) | Expr::String(_) | Expr::Bool(_) | Expr::Null => true,
                Expr::Unary { op, value } => op == "-" && matches!(value.as_ref(), Expr::Number(_)),
                _ => false,
            };
            if !plain { return Err(not_literal()); }
            literals.push(value);
        }
        return Ok(Pattern::Literals(literals));
    }
    if split_top_level(text, ',').len() > 1 {
        return Err("only literals can be listed with commas; for several types write a union, such as [int, float] n".into());
    }
    let (types_text, name) = if let Some(inner) = text.strip_prefix('[') {
        let close = inner.find(']').ok_or("a union of types in a match pattern needs its closing ']'")?;
        (&text[..close + 2], inner[close + 1..].trim())
    } else {
        match text.split_once(char::is_whitespace) {
            Some((ty, name)) => (ty, name.trim()),
            None => (text, ""),
        }
    };
    let types: Vec<String> = match types_text.strip_prefix('[').and_then(|t| t.strip_suffix(']')) {
        Some(list) => list.split(',').map(|t| t.trim().to_string()).collect(),
        None => vec![types_text.to_string()],
    };
    for ty in &types {
        if ty.is_empty() || ty.split('.').count() > 2 || !ty.split('.').all(is_bare_ident) {
            return Err(format!("'{ty}' is not a type a match pattern can test; write a plain type such as int, array, or Circle \
                                (element types, as in array[int], are not checked)"));
        }
    }
    let name = match name {
        "" | "_" => None,
        name if is_bare_ident(name) => {
            reserved(name, "match binding")?;
            Some(name.to_string())
        }
        _ => return Err(format!("expected a type and one name in the match pattern '{text}', such as int n")),
    };
    Ok(Pattern::Type { types, name })
}

/// Arms that can never run: those after an arm without a guard that matches
/// every value (`_` or `any`). A match must also have an arm.
fn check_arms<B>(arms: &[MatchArm<B>]) -> PResult<()> {
    if arms.is_empty() { return Err("this match has no arms; write at least one, such as _ => out(\"anything\");".into()); }
    for (index, arm) in arms.iter().enumerate().take(arms.len() - 1) {
        let catch_all = match &arm.pattern {
            Pattern::Wildcard => Some("_"),
            Pattern::Type { types, .. } if types.iter().any(|t| t == "any") => Some("any"),
            _ => None,
        };
        if let (Some(what), None) = (catch_all, &arm.guard) {
            let next = &arms[index + 1];
            let message = format!("this match arm can never run: the {what} arm before it matches every value, so it must be the last arm");
            return Err(if next.line > 0 { at_line(next.line, message) } else { message });
        }
    }
    Ok(())
}

/// The statement form, from its header `match (x){` at `lines[start]` to its
/// closing brace. Each arm's body is one statement or a `{ }` block.
fn parse_match_stmt(header: &str, lines: &[Line], start: usize) -> PResult<(StmtKind, usize)> {
    let subject = match_subject(header)?;
    let mut arms = Vec::new();
    let mut i = start + 1;
    loop {
        let Some(line) = lines.get(i) else { return Err("unterminated match: its closing '}' is missing".into()) };
        let l = line.trim();
        if l.is_empty() || l == ";" { i += 1; continue; }
        if l == "}" || l == "};" { i += 1; break; }
        let number = line.number;
        // The last arm may end at the match's closing brace instead of a ';'.
        let (text, closes) = match l.strip_suffix('}') { Some(arm) => (arm.trim(), true), None => (l, false) };
        let (pattern, guard, body, next) = parse_stmt_arm(text, lines, i).map_err(|e| at_line(number, e))?;
        arms.push(MatchArm { pattern, guard, body, line: number });
        i = next;
        if closes { break; }
    }
    check_arms(&arms)?;
    Ok((StmtKind::Match { subject, arms }, i))
}

fn parse_stmt_arm(text: &str, lines: &[Line], i: usize) -> PResult<(Pattern, Option<Expr>, Vec<Stmt>, usize)> {
    let (head, rest) = split_arm(text).ok_or(NO_ARROW)?;
    let (pattern, guard) = parse_arm_head(head)?;
    let rest = rest.trim();
    if rest == "{" {
        let mut next = i + 1;
        let body = parse_block_stmts(lines, &mut next)?;
        return Ok((pattern, guard, body, next));
    }
    if rest.is_empty() || rest == ";" {
        return Err("a match arm needs a body after '=>' on the same line: one statement ending in ';', or a { } block".into());
    }
    let (kind, next) = parse_stmt_text(rest, lines, i)?;
    Ok((pattern, guard, vec![Stmt { kind, line: lines[i].number }], next))
}

/// The expression form over several lines, from its header (`text[at..]`, as
/// in `match (x){`) at `lines[idx]` to its closing brace and an optional `;`
/// after it. Each arm's body is one expression.
fn parse_match_expression_lines(header: &str, lines: &[Line], idx: usize) -> PResult<(Expr, usize)> {
    let subject = match_subject(header)?;
    let mut arms = Vec::new();
    let mut i = idx + 1;
    loop {
        let Some(line) = lines.get(i) else { return Err("unterminated match: its closing '}' is missing".into()) };
        let l = line.trim();
        if l.is_empty() || l == ";" { i += 1; continue; }
        if l == "}" || l == "};" { i += 1; break; }
        let number = line.number;
        let (text, closes) = match l.strip_suffix('}') { Some(arm) => (arm.trim(), true), None => (l, false) };
        let arm = (|| {
            let (head, rest) = split_arm(text).ok_or(NO_ARROW)?;
            let (pattern, guard) = parse_arm_head(head)?;
            let rest = rest.trim();
            let rest = rest.strip_suffix(';').unwrap_or(rest).trim();
            if rest.is_empty() {
                return Err("a match arm needs a value after '=>' on the same line".to_string());
            }
            let (body, next) = match match_expression_start(rest).filter(|_| !closes) {
                Some(at) => gather_match_expression(rest, at, lines, i)?,
                None => (parse_expr(rest)?, i + 1),
            };
            Ok((MatchArm { pattern, guard, body, line: number }, next))
        })().map_err(|e| at_line(number, e))?;
        arms.push(arm.0);
        i = arm.1;
        if closes { break; }
    }
    check_arms(&arms)?;
    Ok((Expr::Match { subject: Box::new(subject), arms }, i))
}

/// The expression `text` whose match, starting at `text[at..]`, continues on
/// the lines after `lines[idx]`; what comes before the match, as in
/// `count + match (x){`, is part of it. Also the index of the line after it.
fn gather_match_expression(text: &str, at: usize, lines: &[Line], idx: usize) -> PResult<(Expr, usize)> {
    let (found, mut next) = parse_match_expression_lines(&text[at..], lines, idx)?;
    if lines.get(next).is_some_and(|line| line.trim() == ";") { next += 1; }
    let prefix = text[..at].trim();
    if prefix.is_empty() { return Ok((found, next)); }
    let mut expr = parse_expr(&format!("{prefix} {MATCH_PLACEHOLDER}"))?;
    let mut found = Some(found);
    substitute(&mut expr, &mut found);
    Ok((expr, next))
}

/// A statement such as `label = match (x){` whose match expression continues
/// on the following lines.
fn parse_match_expression_stmt(l: &str, at: usize, lines: &[Line], idx: usize) -> PResult<(StmtKind, usize)> {
    let (found, mut next) = parse_match_expression_lines(&l[at..], lines, idx)?;
    if lines.get(next).is_some_and(|line| line.trim() == ";") { next += 1; }
    let mut kind = parse_stmt_line(&format!("{} {MATCH_PLACEHOLDER};", &l[..at]))?;
    let mut found = Some(found);
    match &mut kind {
        StmtKind::Var(v) => substitute(&mut v.value, &mut found),
        StmtKind::Assign { value, .. } => substitute(value, &mut found),
        StmtKind::Ret(e) | StmtKind::Expr(e) => substitute(e, &mut found),
        _ => {}
    }
    if found.is_some() { return Err("a match expression must be part of a value, as in label = match (x){ ... };".into()); }
    Ok((kind, next))
}

/// Put `found` where the placeholder name stands in `expr`.
fn substitute(expr: &mut Expr, found: &mut Option<Expr>) {
    if found.is_none() { return; }
    match expr {
        Expr::Ident(name) if name == MATCH_PLACEHOLDER => *expr = found.take().unwrap(),
        Expr::Typed { value, .. } | Expr::Freeze(value) | Expr::Unary { value, .. } => substitute(value, found),
        Expr::Binary { left, right, .. } => { substitute(left, found); substitute(right, found); }
        Expr::Call { callee, args } => { substitute(callee, found); for arg in args { substitute(arg, found); } }
        Expr::Member { object, .. } => substitute(object, found),
        Expr::Index { object, index } => { substitute(object, found); substitute(index, found); }
        Expr::Template(parts) => for part in parts { substitute(part, found); },
        // `f = func(int x) => match (x){`: the match is the lambda's body.
        Expr::Lambda(decl) => {
            if let FuncBody::Expr(body) = &mut std::sync::Arc::make_mut(decl).body { substitute(body, found); }
        }
        _ => {}
    }
}

/// A match expression written within one line, as `match (subject){ body }`.
pub(crate) fn match_expression(subject: &str, body: &str) -> PResult<Expr> {
    let subject = subject.trim();
    if subject.is_empty() { return Err("match needs the value to match between its parentheses".into()); }
    let subject = parse_expr(subject)?;
    let mut arms = Vec::new();
    for part in split_top_level(body, ';') {
        let part = part.trim();
        if part.is_empty() { continue; }
        let (head, rest) = split_arm(part).ok_or(NO_ARROW)?;
        let (pattern, guard) = parse_arm_head(head)?;
        if rest.trim().is_empty() { return Err("a match arm needs a value after '=>'".into()); }
        arms.push(MatchArm { pattern, guard, body: parse_expr(rest.trim())?, line: 0 });
    }
    check_arms(&arms)?;
    Ok(Expr::Match { subject: Box::new(subject), arms })
}

fn parse_for_sequence(text: &str, line: usize) -> PResult<Vec<Stmt>> {
    let text = if text.starts_with('(') && find_matching_paren(text)? == text.len() - 1 {
        &text[1..text.len()-1]
    } else { text };
    if text.trim().is_empty() { return Ok(Vec::new()); }
    let mut parts = Vec::new(); let mut start = 0;
    let mut quote = None; let mut escaped = false; let mut depth = 0usize;
    for (i,ch) in text.char_indices() {
        if let Some(q) = quote {
            if escaped { escaped = false; } else if ch == '\\' { escaped = true; } else if ch == q { quote = None; }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' | '[' => depth += 1, ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => return Err("use 'also', not commas, inside grouped for sections".into()),
            _ => {}
        }
        if depth == 0 && text[i..].starts_with(" also ") {
            parts.push(&text[start..i]); start = i + 6;
        }
    }
    parts.push(&text[start..]);
    parts.into_iter().map(|part| {
        let kind = parse_stmt_line(&format!("{};", part.trim()))?;
        if !matches!(kind, StmtKind::Var(_) | StmtKind::Assign { .. } | StmtKind::Expr(_)) {
            return Err("invalid for initializer/change".into());
        }
        Ok(Stmt { kind, line })
    }).collect()
}

fn parse_if_stmt(line: &str, lines: &[Line], start: usize) -> PResult<(StmtKind, usize)> {
    if !line.ends_with('{') {
        return Err("if statement header must end with '{'".to_string());
    }
    let cond = parse_condition_between_parens(line, "if")?;

    let mut i = start + 1;
    let then_block = parse_block_stmts(lines, &mut i)?;

    let mut elif_blocks = Vec::new();
    let mut else_block = None;

    while i < lines.len() {
        let l = lines[i].trim();
        if l.is_empty() {
            i += 1;
            continue;
        }

        if l.starts_with("elif ") || l.starts_with("elif(") {
            if !l.ends_with('{') {
                return Err("elif statement header must end with '{'".to_string());
            }
            let ec = parse_condition_between_parens(l, "elif")?;
            i += 1;
            let eb = parse_block_stmts(lines, &mut i)?;
            elif_blocks.push((ec, eb));
            continue;
        }

        if l.starts_with("else") {
            if !l.ends_with('{') {
                return Err("else statement header must end with '{'".to_string());
            }
            i += 1;
            else_block = Some(parse_block_stmts(lines, &mut i)?);
            break;
        }

        break;
    }

    Ok((
        StmtKind::If {
            cond,
            then_block,
            elif_blocks,
            else_block,
        },
        i,
    ))
}

fn parse_repeat_stmt(line: &str, lines: &[Line], start: usize) -> PResult<(StmtKind, usize)> {
    if !line.ends_with('{') {
        return Err("repeat statement header must end with '{'".to_string());
    }
    let expr_text = line
        .trim_start_matches("repeat")
        .trim()
        .trim_end_matches('{')
        .trim();

    let mut i = start + 1;
    let body = parse_block_stmts(lines, &mut i)?;
    Ok((
        StmtKind::Repeat {
            times: parse_expr(expr_text)?,
            body,
        },
        i,
    ))
}

fn parse_condition_between_parens(line: &str, keyword: &str) -> PResult<Expr> {
    let after_kw = line
        .trim_start_matches(keyword)
        .trim()
        .trim_end_matches('{')
        .trim();
    if !after_kw.starts_with('(') {
        return Err(format!("{keyword} must start condition with '('"));
    }
    let close = find_matching_paren(after_kw)?;
    let cond = &after_kw[1..close];
    parse_expr(cond)
}

fn find_matching_paren(text: &str) -> PResult<usize> {
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    for (i, ch) in text.char_indices() {
        if let Some(q) = quote {
            if escaped { escaped = false; }
            else if ch == '\\' { escaped = true; }
            else if ch == q { quote = None; }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok(i);
                }
            }
            _ => {}
        }
    }
    Err("missing closing ')'".to_string())
}

fn is_var_decl(line: &str) -> bool {
    if !line.ends_with(';') {
        return false;
    }
    let Some(eq) = find_plain_assignment(line) else { return false; };
    let lhs = line[..eq].trim();

    line.starts_with("fixed ")
        || line.starts_with("hot ")
        || line.starts_with("cold ")
        || lhs.split_once('(').map(|(name, _)| is_bare_ident(name.trim())).unwrap_or(false)
        || is_bare_ident(lhs)
}

fn is_bare_ident(text: &str) -> bool {
    let mut chars = text.chars();
    match chars.next() {
        Some(c) if c == '_' || c.is_ascii_alphabetic() => {}
        _ => return false,
    }

    chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

fn parse_var_decl(line: &str) -> PResult<VarDecl> {
    let mut left_right = line.trim_end_matches(';').splitn(2, '=');
    let left = left_right
        .next()
        .ok_or_else(|| "missing lhs".to_string())?
        .trim();
    let value = left_right
        .next()
        .ok_or_else(|| "missing rhs".to_string())?
        .trim();

    let mut tokens = left.split_whitespace().collect::<Vec<_>>();
    let mut is_fixed = false;
    let mut temp = TempKind::Default;

    if tokens.first() == Some(&"fixed") {
        is_fixed = true;
        tokens.remove(0);
    }
    if tokens.first() == Some(&"hot") {
        temp = TempKind::Hot;
        tokens.remove(0);
    } else if tokens.first() == Some(&"cold") {
        temp = TempKind::Cold;
        tokens.remove(0);
    }

    let joined = tokens.join(" ");
    let (name, ty) = if let Some((n, t)) = joined.split_once('(') {
        (
            n.trim().to_string(),
            Some(t.trim_end_matches(')').trim().to_string()),
        )
    } else {
        (joined.trim().to_string(), None)
    };

    if !is_bare_ident(&name) {
        return Err(format!("invalid variable declaration '{name}'; fixed must precede hot/cold"));
    }
    reserved(&name, "variable")?;

    Ok(VarDecl {
        is_fixed,
        temp,
        name,
        ty,
        value: parse_expr(value)?,
        line: 0,
    })
}

fn parse_stmt_line(line: &str) -> PResult<StmtKind> {
    let l = line.trim();
    if l == "break;" { return Ok(StmtKind::Break); }
    if l == "continue;" { return Ok(StmtKind::Continue); }
    for (suffix, op) in [("++;", "+"), ("--;", "-")] {
        if let Some(target) = l.strip_suffix(suffix) {
            let target = parse_expr(target)?;
            return Ok(StmtKind::Assign { target: target.clone(), value: Expr::Binary {
                left: Box::new(target), op: op.into(), right: Box::new(Expr::Number("1".into())),
            }});
        }
    }
    for op in ["+=", "-="] {
        if let Some((target, value)) = l.strip_suffix(';').and_then(|s| s.split_once(op)) {
            if is_bare_ident(target.trim()) {
                let target = parse_expr(target)?;
                return Ok(StmtKind::Assign { target: target.clone(), value: Expr::Binary {
                    left: Box::new(target), op: op[..1].into(), right: Box::new(parse_expr(value)?),
                }});
            }
        }
    }
    if l.starts_with("ret ") {
        return Ok(StmtKind::Ret(parse_expr(
            l.trim_start_matches("ret").trim().trim_end_matches(';'),
        )?));
    }
    if is_var_decl(l) {
        return Ok(StmtKind::Var(parse_var_decl(l)?));
    }
    if let Some(eq_idx) = find_plain_assignment(l.trim_end_matches(';')) {
        let (lhs, rhs_with_eq) = l.trim_end_matches(';').split_at(eq_idx);
        let rhs = &rhs_with_eq[1..];
        return Ok(StmtKind::Assign {
            target: parse_expr(lhs.trim())?,
            value: parse_expr(rhs.trim())?,
        });
    }
    Ok(StmtKind::Expr(parse_expr(l.trim_end_matches(';'))?))
}

fn find_plain_assignment(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    let mut previous = None;
    let mut chars = text.char_indices().peekable();
    while let Some((i, ch)) = chars.next() {
        if let Some(q) = quote {
            if escaped { escaped = false; }
            else if ch == '\\' { escaped = true; }
            else if ch == q { quote = None; }
            previous = Some(ch);
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            '=' if depth == 0 => {
                let prev = previous;
                let next = chars.peek().map(|(_, ch)| *ch);
                if prev != Some('=')
                    && prev != Some('!')
                    && prev != Some('<')
                    && prev != Some('>')
                    && prev != Some('+')
                    && prev != Some('-')
                    && next != Some('=')
                {
                    return Some(i);
                }
            }
            _ => {}
        }
        previous = Some(ch);
    }
    None
}

fn parse_expr(text: &str) -> PResult<Expr> {
    crate::expression::parse(text)
}

fn split_top_level(text: &str, delimiter: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut paren = 0usize;
    let mut bracket = 0usize;
    let mut brace = 0usize;
    let mut in_string: Option<char> = None;
    let mut escaped = false;
    let chars: Vec<char> = text.chars().collect();

    let mut i = 0usize;
    while i < chars.len() {
        let ch = chars[i];
        if let Some(q) = in_string {
            if escaped { escaped = false; }
            else if ch == '\\' { escaped = true; }
            else if ch == q {
                in_string = None;
            }
            i += 1;
            continue;
        }

        match ch {
            '"' | '\'' => in_string = Some(ch),
            '(' => paren += 1,
            ')' => paren = paren.saturating_sub(1),
            '[' => bracket += 1,
            ']' => bracket = bracket.saturating_sub(1),
            '{' => brace += 1,
            '}' => brace = brace.saturating_sub(1),
            _ => {}
        }

        if ch == delimiter && paren == 0 && bracket == 0 && brace == 0 {
            parts.push(chars[start..i].iter().collect());
            start = i + 1;
        }

        i += 1;
    }

    parts.push(chars[start..].iter().collect());
    parts
}
