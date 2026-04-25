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
        if is_var_decl(line) {
            items.push(Item::Var(parse_var_decl(line)?));
            idx += 1;
            continue;
        }
        items.push(Item::Stmt(parse_stmt(line)?));
        idx += 1;
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
        if line.starts_with("func ") {
            let (func, next) = parse_func(lines, i)?;
            methods.push(func);
            i = next;
            continue;
        }
        i += 1;
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
    let mut body = Vec::new();
    let mut i = start + 1;
    while i < lines.len() {
        let line = lines[i].trim();
        if line == "}" || line == "};" {
            return Ok((
                FuncDecl {
                    name,
                    params,
                    body: FuncBody::Block(body),
                },
                i + 1,
            ));
        }
        if line.is_empty() {
            i += 1;
            continue;
        }
        body.push(parse_stmt(line)?);
        i += 1;
    }
    Err("unterminated function block".to_string())
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

fn is_var_decl(line: &str) -> bool {
    (line.contains("=") && line.ends_with(';'))
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

fn parse_stmt(line: &str) -> PResult<Stmt> {
    let l = line.trim();
    if l.starts_with("ret ") {
        return Ok(Stmt::Ret(parse_expr(
            l.trim_start_matches("ret").trim().trim_end_matches(';'),
        )));
    }
    if l.starts_with("if ") {
        return Ok(Stmt::Expr(Expr::Raw(l.to_string())));
    }
    if l.starts_with("repeat ") {
        return Ok(Stmt::Expr(Expr::Raw(l.to_string())));
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
    if let Some(dot) = t.find('.') {
        let (obj, field) = t.split_at(dot);
        if !obj.is_empty() && !field.is_empty() && !t.contains('(') {
            return Expr::Member {
                object: Box::new(parse_expr(obj)),
                field: field.trim_start_matches('.').to_string(),
            };
        }
    }
    if let Some(p) = t.find('(') {
        if t.ends_with(')') {
            let callee = t[..p].trim();
            let args = t[p + 1..t.len() - 1]
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(parse_expr)
                .collect::<Vec<_>>();
            return Expr::Call {
                callee: Box::new(parse_expr(callee)),
                args,
            };
        }
    }
    Expr::Ident(t.to_string())
}
