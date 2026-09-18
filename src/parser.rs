use crate::ast::*;

pub type PResult<T> = Result<T, String>;

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
        if line.starts_with("takepkg ") {
            items.push(Item::Import(parse_import(line)?));
            idx += 1;
            continue;
        }
        if line.starts_with("family ") {
            let (family, next) = parse_family(&lines, idx)?;
            items.push(Item::Family(family));
            idx = next;
            continue;
        }
        if line.starts_with("func ") {
            let (func, next) = parse_func(&lines, idx)?;
            items.push(Item::Func(func));
            idx = next;
            continue;
        }
        let (stmt, next) = parse_stmt_at(&lines, idx)?;
        if let Stmt::Var(v) = stmt {
            items.push(Item::Var(v));
        } else {
            items.push(Item::Stmt(stmt));
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
        match stmt {
            Stmt::Break | Stmt::Continue if depth == 0 => {
                return Err("break/continue must be inside a loop".to_string());
            }
            Stmt::Repeat { body, .. } | Stmt::While { body, .. } |
            Stmt::ForEach { body, .. } | Stmt::For { body, .. } => validate_loop_control(body, depth + 1)?,
            Stmt::If { then_block, elif_blocks, else_block, .. } => {
                validate_loop_control(then_block, depth)?;
                for (_, body) in elif_blocks { validate_loop_control(body, depth)?; }
                if let Some(body) = else_block { validate_loop_control(body, depth)?; }
            }
            _ => {}
        }
    }
    Ok(())
}

fn preprocess(input: &str) -> PResult<Vec<String>> {
    let mut clean = String::new();
    let mut chars = input.chars().peekable();
    let mut quote = None;
    let mut escaped = false;
    let mut block_start = None;
    let mut line_comment = false;
    let (mut line, mut column) = (1usize, 0usize);
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
        } else {
            if ch == '\'' || ch == '"' {
                quote = Some(ch);
            }
            clean.push(ch);
        }
    }
    if let Some((line, column)) = block_start {
        return Err(format!("unterminated block comment at {line}:{column}"));
    }
    if quote.is_some() {
        return Err(format!("unterminated string at {line}:{column}"));
    }
    // Layout does not delimit statements. Split only outside strings and
    // parenthesized headers/calls, so compact blocks use the same parser path.
    let mut normalized = String::new();
    let mut quote = None;
    let mut escaped = false;
    let mut depth = 0usize;
    for ch in clean.chars() {
        normalized.push(ch);
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
            '{' | '}' | ';' if depth == 0 => normalized.push('\n'),
            _ => {}
        }
    }
    Ok(normalized.lines().map(str::to_string).collect())
}

fn parse_import(line: &str) -> PResult<ImportDecl> {
    let body = line
        .trim_end_matches(';')
        .trim_start_matches("takepkg")
        .trim();
    if let Some((module, alias)) = body.split_once('=') {
        Ok(ImportDecl {
            module: module.trim().to_string(),
            alias: Some(alias.trim().to_string()),
        })
    } else {
        Ok(ImportDecl {
            module: body.to_string(),
            alias: None,
        })
    }
}

fn parse_family(lines: &[String], start: usize) -> PResult<(FamilyDecl, usize)> {
    let header = lines[start].trim();
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

    let mut methods = Vec::new();
    let mut i = start + 1;
    while i < lines.len() {
        let line = lines[i].trim();
        if line == "}" || line == "};" {
            return Ok((
                FamilyDecl {
                    name,
                    extends,
                    methods,
                },
                i + 1,
            ));
        }
        if line.is_empty() || line == ";" {
            i += 1;
            continue;
        }
        if line.starts_with("func ") {
            let (func, next) = parse_func(lines, i)?;
            methods.push(func);
            i = next;
            continue;
        }
        return Err(format!("unexpected token in family body: '{line}'"));
    }
    Err("unterminated family block".to_string())
}

fn parse_func(lines: &[String], start: usize) -> PResult<(FuncDecl, usize)> {
    let header = lines[start].trim();
    if header.contains("=>") {
        let (left, right) = header
            .trim_end_matches(';')
            .split_once("=>")
            .ok_or_else(|| "invalid single-line function".to_string())?;
        let (name, params) = parse_func_signature(left.trim_start_matches("func").trim())?;
        return Ok((
            FuncDecl {
                name,
                params,
                body: FuncBody::Expr(parse_expr(right.trim())?),
            },
            start + 1,
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
        },
        i,
    ))
}

fn parse_block_stmts(lines: &[String], idx: &mut usize) -> PResult<Vec<Stmt>> {
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

fn parse_stmt_at(lines: &[String], idx: usize) -> PResult<(Stmt, usize)> {
    let l = lines[idx].trim();

    if l.starts_with("while ") || l.starts_with("while(") {
        if !l.ends_with('{') { return Err("while must open a block".into()); }
        let cond = parse_condition_between_parens(l, "while")?;
        let mut next = idx + 1;
        let body = parse_block_stmts(lines, &mut next)?;
        return Ok((Stmt::While { cond, body }, next));
    }
    if l.starts_with("for ") || l.starts_with("for(") {
        let header = l.trim_start_matches("for").trim().strip_suffix('{')
            .ok_or("for must open a block")?.trim();
        let header = header.strip_prefix('(').and_then(|s| s.strip_suffix(')'))
            .ok_or("for requires a parenthesized header")?;
        let parts = split_top_level(header, ',');
        if parts.len() == 3 {
            let init = parse_for_sequence(parts[0].trim())?;
            let cond = parse_expr(parts[1].trim())?;
            let step = parse_for_sequence(parts[2].trim())?;
            let mut next = idx + 1;
            let body = parse_block_stmts(lines, &mut next)?;
            return Ok((Stmt::For { init, cond, step, body }, next));
        }
        if parts.len() != 2 {
            return Err("expected for (item, iterable) or for (initializer, condition, change)".into());
        }
        let name = parts[0].trim();
        if !is_bare_ident(name) || parts[1].trim().is_empty() {
            return Err("expected for (itemname, iterable)".into());
        }
        let iterable = parse_expr(parts[1].trim())?;
        let mut next = idx + 1;
        let body = parse_block_stmts(lines, &mut next)?;
        return Ok((Stmt::ForEach { name: name.into(), iterable, body }, next));
    }

    if l.starts_with("if ") || l.starts_with("if(") {
        return parse_if_stmt(lines, idx);
    }
    if l.starts_with("repeat ") {
        return parse_repeat_stmt(lines, idx);
    }
    Ok((parse_stmt_line(l)?, idx + 1))
}

fn parse_for_sequence(text: &str) -> PResult<Vec<Stmt>> {
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
        let stmt = parse_stmt_line(&format!("{};", part.trim()))?;
        if !matches!(stmt, Stmt::Var(_) | Stmt::Assign { .. } | Stmt::Expr(_)) {
            return Err("invalid for initializer/change".into());
        }
        Ok(stmt)
    }).collect()
}

fn parse_if_stmt(lines: &[String], start: usize) -> PResult<(Stmt, usize)> {
    let line = lines[start].trim();
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
        Stmt::If {
            cond,
            then_block,
            elif_blocks,
            else_block,
        },
        i,
    ))
}

fn parse_repeat_stmt(lines: &[String], start: usize) -> PResult<(Stmt, usize)> {
    let line = lines[start].trim();
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
        Stmt::Repeat {
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

    Ok(VarDecl {
        is_fixed,
        temp,
        name,
        ty,
        value: parse_expr(value)?,
    })
}

fn parse_stmt_line(line: &str) -> PResult<Stmt> {
    let l = line.trim();
    if l == "break;" { return Ok(Stmt::Break); }
    if l == "continue;" { return Ok(Stmt::Continue); }
    for (suffix, op) in [("++;", "+"), ("--;", "-")] {
        if let Some(target) = l.strip_suffix(suffix) {
            let target = parse_expr(target)?;
            return Ok(Stmt::Assign { target: target.clone(), value: Expr::Binary {
                left: Box::new(target), op: op.into(), right: Box::new(Expr::Number("1".into())),
            }});
        }
    }
    for op in ["+=", "-="] {
        if let Some((target, value)) = l.strip_suffix(';').and_then(|s| s.split_once(op)) {
            if is_bare_ident(target.trim()) {
                let target = parse_expr(target)?;
                return Ok(Stmt::Assign { target: target.clone(), value: Expr::Binary {
                    left: Box::new(target), op: op[..1].into(), right: Box::new(parse_expr(value)?),
                }});
            }
        }
    }
    if l.starts_with("ret ") {
        return Ok(Stmt::Ret(parse_expr(
            l.trim_start_matches("ret").trim().trim_end_matches(';'),
        )?));
    }
    if is_var_decl(l) {
        return Ok(Stmt::Var(parse_var_decl(l)?));
    }
    if let Some(eq_idx) = find_plain_assignment(l.trim_end_matches(';')) {
        let (lhs, rhs_with_eq) = l.trim_end_matches(';').split_at(eq_idx);
        let rhs = &rhs_with_eq[1..];
        return Ok(Stmt::Assign {
            target: parse_expr(lhs.trim())?,
            value: parse_expr(rhs.trim())?,
        });
    }
    Ok(Stmt::Expr(parse_expr(l.trim_end_matches(';'))?))
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
