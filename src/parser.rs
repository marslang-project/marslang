use crate::ast::*;

pub type PResult<T> = Result<T, String>;

pub fn parse_program(input: &str) -> PResult<Program> {
    let lines = preprocess(input);
    let mut idx = 0usize;
    let mut items = Vec::new();
    while idx < lines.len() {
        let line = lines[idx].trim();
        if line.is_empty() {
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

    Ok(Program { items })
}

fn preprocess(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_block_comment = false;
    for raw in input.lines() {
        let mut line = raw.to_string();
        if in_block_comment {
            if let Some(end) = line.find("*/") {
                line = line[end + 2..].to_string();
                in_block_comment = false;
            } else {
                continue;
            }
        }
        while let Some(start) = line.find("/*") {
            if let Some(end_rel) = line[start + 2..].find("*/") {
                let end = start + 2 + end_rel;
                line.replace_range(start..end + 2, " ");
            } else {
                line.truncate(start);
                in_block_comment = true;
                break;
            }
        }
        if let Some(pos) = line.find("//") {
            line.truncate(pos);
        }
        out.push(line);
    }
    out
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
        if line.is_empty() {
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
                body: FuncBody::Expr(parse_expr(right.trim())),
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
        if line.is_empty() {
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
    if let Some((name, rest)) = sig.split_once('(') {
        let params_part = rest.trim_end_matches(')').trim();
        let mut params = Vec::new();
        for p in params_part
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let mut parts = p.split_whitespace();
            let ty = parts
                .next()
                .ok_or_else(|| format!("missing param type in '{p}'"))?;
            let name = parts
                .next()
                .ok_or_else(|| format!("missing param name in '{p}'"))?;
            params.push(Param {
                ty: ty.to_string(),
                name: name.to_string(),
            });
        }
        Ok((name.trim().to_string(), params))
    } else {
        Ok((sig.trim().to_string(), Vec::new()))
    }
}

fn parse_stmt_at(lines: &[String], idx: usize) -> PResult<(Stmt, usize)> {
    let l = lines[idx].trim();

    if l.starts_with("if ") {
        return parse_if_stmt(lines, idx);
    }
    if l.starts_with("repeat ") {
        return parse_repeat_stmt(lines, idx);
    }
    Ok((parse_stmt_line(l)?, idx + 1))
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

        if l.starts_with("elif ") {
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
            times: parse_expr(expr_text),
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
    Ok(parse_expr(cond))
}

fn find_matching_paren(text: &str) -> PResult<usize> {
    let mut depth = 0usize;
    for (i, ch) in text.char_indices() {
        match ch {
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
    (line.contains('=') && line.ends_with(';'))
        && (line.starts_with("fixed ")
            || line.starts_with("hot ")
            || line.starts_with("cold ")
            || line.contains(" (")
            || (line
                .split('=')
                .next()
                .unwrap_or_default()
                .trim()
                .contains(' ')))
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

    Ok(VarDecl {
        is_fixed,
        temp,
        name,
        ty,
        value: parse_expr(value),
    })
}

fn parse_stmt_line(line: &str) -> PResult<Stmt> {
    let l = line.trim();
    if l.starts_with("ret ") {
        return Ok(Stmt::Ret(parse_expr(
            l.trim_start_matches("ret").trim().trim_end_matches(';'),
        )));
    }
    if is_var_decl(l) {
        return Ok(Stmt::Var(parse_var_decl(l)?));
    }
    if let Some(eq_idx) = find_plain_assignment(l.trim_end_matches(';')) {
        let (lhs, rhs_with_eq) = l.trim_end_matches(';').split_at(eq_idx);
        let rhs = &rhs_with_eq[1..];
        return Ok(Stmt::Assign {
            target: parse_expr(lhs.trim()),
            value: parse_expr(rhs.trim()),
        });
    }
    Ok(Stmt::Expr(parse_expr(l.trim_end_matches(';'))))
}

fn find_plain_assignment(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    let chars: Vec<char> = text.chars().collect();
    for (i, ch) in chars.iter().enumerate() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            '=' if depth == 0 => {
                let prev = if i > 0 { Some(chars[i - 1]) } else { None };
                let next = chars.get(i + 1).copied();
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
    }
    None
}

fn parse_expr(text: &str) -> Expr {
    let t = text.trim();
    if t == "true" {
        return Expr::Bool(true);
    }
    if t == "false" || t == "fasle" {
        return Expr::Bool(false);
    }
    if t == "null" {
        return Expr::Null;
    }
    if t.parse::<f64>().is_ok() {
        return Expr::Number(t.to_string());
    }
    if (t.starts_with('"') && t.ends_with('"')) || (t.starts_with('\'') && t.ends_with('\'')) {
        return Expr::String(t.to_string());
    }
    if let Some(p) = t.find('(') {
        if t.ends_with(')') {
            let callee = t[..p].trim();
            let args_text = &t[p + 1..t.len() - 1];
            let args = split_top_level(args_text, ',')
                .into_iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .map(|s| parse_expr(&s))
                .collect::<Vec<_>>();
            return Expr::Call {
                callee: Box::new(parse_expr(callee)),
                args,
            };
        }
    }
    if let Some(dot) = t.find('.') {
        let (obj, field) = t.split_at(dot);
        if !obj.is_empty() && !field.is_empty() && !t.contains(' ') {
            return Expr::Member {
                object: Box::new(parse_expr(obj)),
                field: field.trim_start_matches('.').to_string(),
            };
        }
    }
    Expr::Ident(t.to_string())
}

fn split_top_level(text: &str, delimiter: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut paren = 0usize;
    let mut bracket = 0usize;
    let mut brace = 0usize;
    let mut in_string: Option<char> = None;
    let chars: Vec<char> = text.chars().collect();

    let mut i = 0usize;
    while i < chars.len() {
        let ch = chars[i];
        if let Some(q) = in_string {
            if ch == q && (i == 0 || chars[i - 1] != '\\') {
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
