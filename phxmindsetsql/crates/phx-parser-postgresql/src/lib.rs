use phx_sql_model::{
    canonical_name, CheckConstraint, Column, ForeignKey, Migration, ObjectKind, Partitioning, SqlModel, SqlObject, SqlStats, Table,
};
use regex::Regex;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Clone, Debug)]
struct StatementMatch {
    kind: ObjectKind,
    name: String,
    target: Option<String>,
    start: usize,
    end: usize,
}

pub fn parse_sql(source_name: impl Into<String>, sql: String) -> SqlModel {
    let source_name = source_name.into();
    let line_starts = line_starts(&sql);
    let migrations = parse_migrations(&sql, &line_starts);
    let mut tables = parse_tables(&sql, &line_starts, &migrations);
    let mut relationships = Vec::new();

    for table in &tables {
        relationships.extend(table.foreign_keys.clone());
    }

    let alter_fks = parse_alter_foreign_keys(&sql, &line_starts);
    let mut signatures: HashSet<String> = relationships.iter().map(ForeignKey::signature).collect();
    for fk in alter_fks {
        if signatures.insert(fk.signature()) {
            relationships.push(fk);
        }
    }

    let table_lookup: HashMap<String, usize> = tables
        .iter()
        .enumerate()
        .map(|(i, t)| (canonical_name(&t.name), i))
        .collect();
    for fk in &relationships {
        if let Some(&idx) = table_lookup.get(&canonical_name(&fk.from_table)) {
            if !tables[idx]
                .foreign_keys
                .iter()
                .any(|existing| existing.signature() == fk.signature())
            {
                tables[idx].foreign_keys.push(fk.clone());
            }
        }
    }

    let mut matches = Vec::new();
    for table in &tables {
        let start = byte_offset_for_line(&line_starts, table.line);
        let end = byte_offset_for_line(&line_starts, table.end_line + 1).min(sql.len());
        matches.push(StatementMatch {
            kind: ObjectKind::Table,
            name: table.name.clone(),
            target: None,
            start,
            end,
        });
    }

    matches.extend(find_named_statements(&sql, ObjectKind::MaterializedView));
    matches.extend(find_named_statements(&sql, ObjectKind::View));
    matches.extend(find_named_statements(&sql, ObjectKind::Sequence));
    matches.extend(find_named_statements(&sql, ObjectKind::Extension));
    matches.extend(find_named_statements(&sql, ObjectKind::Partition));
    matches.extend(find_named_statements(&sql, ObjectKind::Function));
    matches.extend(find_named_statements(&sql, ObjectKind::Trigger));
    matches.extend(find_named_statements(&sql, ObjectKind::Index));
    matches.extend(find_named_statements(&sql, ObjectKind::Policy));
    matches.extend(find_named_statements(&sql, ObjectKind::Schema));
    matches.sort_by_key(|m| m.start);

    let objects = matches
        .into_iter()
        .enumerate()
        .map(|(id, m)| {
            let line = line_of(&line_starts, m.start);
            let end_line = line_of(&line_starts, m.end.saturating_sub(1));
            SqlObject {
                id,
                kind: m.kind,
                name: m.name,
                line,
                end_line,
                migration: migration_for_line(&migrations, line),
                target: m.target,
                raw_sql: sql[m.start..m.end.min(sql.len())].trim().to_string(),
                metadata: BTreeMap::new(),
            }
        })
        .collect::<Vec<_>>();

    let stats = SqlStats {
        lines: sql.lines().count(),
        bytes: sql.len(),
        tables: tables.len(),
        relationships: relationships.len(),
        columns: tables.iter().map(|t| t.columns.len()).sum(),
        check_constraints: tables.iter().map(|t| t.check_constraints.len()).sum(),
        generated_columns: tables.iter().flat_map(|t| t.columns.iter()).filter(|c| c.generated_expression.is_some()).count(),
        views: objects.iter().filter(|o| o.kind == ObjectKind::View).count(),
        materialized_views: objects.iter().filter(|o| o.kind == ObjectKind::MaterializedView).count(),
        sequences: objects.iter().filter(|o| o.kind == ObjectKind::Sequence).count(),
        functions: objects.iter().filter(|o| o.kind == ObjectKind::Function).count(),
        triggers: objects.iter().filter(|o| o.kind == ObjectKind::Trigger).count(),
        indexes: objects.iter().filter(|o| o.kind == ObjectKind::Index).count(),
        policies: objects.iter().filter(|o| o.kind == ObjectKind::Policy).count(),
        schemas: objects.iter().filter(|o| o.kind == ObjectKind::Schema).count(),
        synonyms: 0,
        events: 0,
        extensions: objects.iter().filter(|o| o.kind == ObjectKind::Extension).count(),
        partitions: objects.iter().filter(|o| o.kind == ObjectKind::Partition).count() + tables.iter().filter(|t| t.partitioning.is_some()).count(),
        migrations: migrations.len(),
    };

    SqlModel {
        source_name,
        sql,
        migrations,
        tables,
        relationships,
        objects,
        stats,
    }
}

fn parse_migrations(sql: &str, line_starts: &[usize]) -> Vec<Migration> {
    let re = Regex::new(r"(?m)^--\s*MIGRATION\s+(\d+):\s*([^\r\n]+)").unwrap();
    re.captures_iter(sql)
        .filter_map(|cap| {
            let version = cap.get(1)?.as_str().parse::<u32>().ok()?;
            let name = cap.get(2)?.as_str().trim().to_string();
            let line = line_of(line_starts, cap.get(0)?.start());
            Some(Migration { version, name, line })
        })
        .collect()
}

fn parse_tables(sql: &str, line_starts: &[usize], migrations: &[Migration]) -> Vec<Table> {
    let re = Regex::new(
        r#"(?im)^\s*CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?([A-Za-z_][A-Za-z0-9_$\.\"]*)\s*\("#,
    )
    .unwrap();

    let mut tables = Vec::new();
    for cap in re.captures_iter(sql) {
        let whole = match cap.get(0) {
            Some(v) => v,
            None => continue,
        };
        let name = cap.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
        let open = whole.end().saturating_sub(1);
        let close = find_matching_paren(sql, open).unwrap_or_else(|| find_statement_end(sql, open));
        let stmt_end = find_statement_end(sql, close);
        let line = line_of(line_starts, whole.start());
        let end_line = line_of(line_starts, stmt_end.saturating_sub(1));
        let body = if close > open + 1 { &sql[open + 1..close] } else { "" };
        let mut columns = Vec::new();
        let mut primary_key = Vec::new();
        let mut foreign_keys = Vec::new();
        let mut check_constraints = Vec::new();

        for item in split_top_level(body, ',') {
            let item = item.trim();
            if item.is_empty() {
                continue;
            }
            let upper = item.to_ascii_uppercase();
            if upper.starts_with("CONSTRAINT ")
                || upper.starts_with("PRIMARY KEY")
                || upper.starts_with("FOREIGN KEY")
                || upper.starts_with("UNIQUE")
                || upper.starts_with("CHECK")
                || upper.starts_with("EXCLUDE")
            {
                if let Some(cols) = parse_table_primary_key(item) {
                    for col in cols {
                        if !primary_key.iter().any(|x| canonical_name(x) == canonical_name(&col)) {
                            primary_key.push(col);
                        }
                    }
                }
                if let Some(fk) = parse_fk_clause(&name, item, line) {
                    foreign_keys.push(fk);
                }
                check_constraints.extend(parse_checks(item, None, line));
                continue;
            }

            if let Some(mut column) = parse_column(item) {
                if column.primary_key
                    && !primary_key
                        .iter()
                        .any(|x| canonical_name(x) == canonical_name(&column.name))
                {
                    primary_key.push(column.name.clone());
                }
                if let Some(fk) = parse_inline_reference(&name, &column.name, item, line) {
                    foreign_keys.push(fk);
                }
                check_constraints.extend(parse_checks(item, Some(&column.name), line));
                column.primary_key = column.primary_key
                    || primary_key
                        .iter()
                        .any(|x| canonical_name(x) == canonical_name(&column.name));
                columns.push(column);
            }
        }

        for col in &mut columns {
            if primary_key
                .iter()
                .any(|pk| canonical_name(pk) == canonical_name(&col.name))
            {
                col.primary_key = true;
                col.nullable = false;
            }
        }

        tables.push(Table {
            name,
            line,
            end_line,
            migration: migration_for_line(migrations, line),
            columns,
            primary_key,
            foreign_keys,
            check_constraints,
            partitioning: parse_partitioning(&sql[whole.start()..stmt_end.min(sql.len())]),
            statistics: None,
            raw_sql: sql[whole.start()..stmt_end.min(sql.len())].trim().to_string(),
        });
    }
    tables
}

fn parse_column(item: &str) -> Option<Column> {
    let (name, rest) = take_identifier(item)?;
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }
    let type_end = find_constraint_keyword(rest).unwrap_or(rest.len());
    let data_type = rest[..type_end].trim().to_string();
    if data_type.is_empty() {
        return None;
    }
    let constraints = &rest[type_end..];
    let upper = constraints.to_ascii_uppercase();
    let nullable = !upper.contains("NOT NULL") && !upper.contains("PRIMARY KEY");
    let primary_key = upper.contains("PRIMARY KEY");
    let unique = upper.contains(" UNIQUE") || upper.starts_with("UNIQUE");
    let default_expr = extract_default(constraints);
    Some(Column {
        name,
        data_type,
        nullable,
        default_expr,
        primary_key,
        unique,
        generated_expression: extract_generated(constraints).map(|x| x.0),
        generated_kind: extract_generated(constraints).map(|x| x.1),
    })
}

fn parse_table_primary_key(item: &str) -> Option<Vec<String>> {
    let upper = item.to_ascii_uppercase();
    let pos = upper.find("PRIMARY KEY")?;
    let rest = &item[pos + "PRIMARY KEY".len()..];
    let open = rest.find('(')?;
    let close = rest[open + 1..].find(')')? + open + 1;
    Some(parse_ident_list(&rest[open + 1..close]))
}

fn parse_fk_clause(from_table: &str, item: &str, source_line: usize) -> Option<ForeignKey> {
    let re = Regex::new(
        r#"(?is)^(?:CONSTRAINT\s+([A-Za-z_][A-Za-z0-9_$\"]*)\s+)?FOREIGN\s+KEY\s*\(([^)]*)\)\s+REFERENCES\s+([A-Za-z_][A-Za-z0-9_$\.\"]*)\s*\(([^)]*)\)(.*)$"#,
    )
    .unwrap();
    let cap = re.captures(item.trim())?;
    let tail = cap.get(5).map(|m| m.as_str()).unwrap_or("");
    Some(ForeignKey {
        constraint_name: cap.get(1).map(|m| m.as_str().trim_matches('"').to_string()),
        from_table: from_table.to_string(),
        from_columns: parse_ident_list(cap.get(2)?.as_str()),
        to_table: cap.get(3)?.as_str().to_string(),
        to_columns: parse_ident_list(cap.get(4)?.as_str()),
        on_delete: parse_on_delete(tail),
        source_line,
    })
}

fn parse_inline_reference(
    from_table: &str,
    from_column: &str,
    item: &str,
    source_line: usize,
) -> Option<ForeignKey> {
    let re = Regex::new(
        r#"(?is)\bREFERENCES\s+([A-Za-z_][A-Za-z0-9_$\.\"]*)\s*\(([^)]*)\)(.*)$"#,
    )
    .unwrap();
    let cap = re.captures(item)?;
    let tail = cap.get(3).map(|m| m.as_str()).unwrap_or("");
    Some(ForeignKey {
        constraint_name: None,
        from_table: from_table.to_string(),
        from_columns: vec![from_column.to_string()],
        to_table: cap.get(1)?.as_str().to_string(),
        to_columns: parse_ident_list(cap.get(2)?.as_str()),
        on_delete: parse_on_delete(tail),
        source_line,
    })
}

fn parse_alter_foreign_keys(sql: &str, line_starts: &[usize]) -> Vec<ForeignKey> {
    let re = Regex::new(
        r#"(?is)ALTER\s+TABLE\s+(?:ONLY\s+)?([A-Za-z_][A-Za-z0-9_$\.\"]*)\s+ADD\s+(?:CONSTRAINT\s+([A-Za-z_][A-Za-z0-9_$\"]*)\s+)?FOREIGN\s+KEY\s*\(([^)]*)\)\s+REFERENCES\s+([A-Za-z_][A-Za-z0-9_$\.\"]*)\s*\(([^)]*)\)([^;]*);"#,
    )
    .unwrap();

    re.captures_iter(sql)
        .filter_map(|cap| {
            let whole = cap.get(0)?;
            Some(ForeignKey {
                constraint_name: cap.get(2).map(|m| m.as_str().trim_matches('"').to_string()),
                from_table: cap.get(1)?.as_str().to_string(),
                from_columns: parse_ident_list(cap.get(3)?.as_str()),
                to_table: cap.get(4)?.as_str().to_string(),
                to_columns: parse_ident_list(cap.get(5)?.as_str()),
                on_delete: parse_on_delete(cap.get(6).map(|m| m.as_str()).unwrap_or("")),
                source_line: line_of(line_starts, whole.start()),
            })
        })
        .collect()
}

fn find_named_statements(sql: &str, kind: ObjectKind) -> Vec<StatementMatch> {
    let pattern = match kind {
        ObjectKind::MaterializedView => {
            r#"(?im)^\s*CREATE\s+MATERIALIZED\s+VIEW\s+(?:IF\s+NOT\s+EXISTS\s+)?([A-Za-z_][A-Za-z0-9_$\.\"]*)"#
        }
        ObjectKind::View => {
            r#"(?im)^\s*CREATE\s+(?:OR\s+REPLACE\s+)?VIEW\s+([A-Za-z_][A-Za-z0-9_$\.\"]*)"#
        }
        ObjectKind::Sequence => {
            r#"(?im)^\s*CREATE\s+(?:TEMP(?:ORARY)?\s+)?SEQUENCE\s+(?:IF\s+NOT\s+EXISTS\s+)?([A-Za-z_][A-Za-z0-9_$\.\"]*)"#
        }
        ObjectKind::Extension => {
            r#"(?im)^\s*CREATE\s+EXTENSION\s+(?:IF\s+NOT\s+EXISTS\s+)?([A-Za-z_][A-Za-z0-9_$\.\"]*)"#
        }
        ObjectKind::Partition => {
            r#"(?im)^\s*CREATE\s+(?:UNLOGGED\s+)?TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?([A-Za-z_][A-Za-z0-9_$\.\"]*)\s+PARTITION\s+OF\s+[A-Za-z_]"#
        }
        ObjectKind::Function => {
            r#"(?im)^\s*CREATE\s+(?:OR\s+REPLACE\s+)?FUNCTION\s+([A-Za-z_][A-Za-z0-9_$\.\"]*)"#
        }
        ObjectKind::Trigger => {
            r#"(?im)^\s*CREATE\s+(?:OR\s+REPLACE\s+)?TRIGGER\s+([A-Za-z_][A-Za-z0-9_$\"]*)"#
        }
        ObjectKind::Index => {
            r#"(?im)^\s*CREATE\s+(?:UNIQUE\s+)?INDEX\s+(?:IF\s+NOT\s+EXISTS\s+)?([A-Za-z_][A-Za-z0-9_$\.\"]*)"#
        }
        ObjectKind::Policy => {
            r#"(?im)^\s*CREATE\s+POLICY\s+([A-Za-z_][A-Za-z0-9_$\"]*)"#
        }
        ObjectKind::Schema => {
            r#"(?im)^\s*CREATE\s+SCHEMA\s+(?:IF\s+NOT\s+EXISTS\s+)?([A-Za-z_][A-Za-z0-9_$\"]*)"#
        }
        ObjectKind::Table | ObjectKind::Synonym | ObjectKind::Event => return Vec::new(),
    };
    let re = Regex::new(pattern).unwrap();
    let on_re = Regex::new(r#"(?is)\bON\s+([A-Za-z_][A-Za-z0-9_$\.\"]*)"#).unwrap();
    let partition_re = Regex::new(r#"(?is)\bPARTITION\s+OF\s+([A-Za-z_][A-Za-z0-9_$\.\"]*)"#).unwrap();

    re.captures_iter(sql)
        .filter_map(|cap| {
            let whole = cap.get(0)?;
            let start = whole.start();
            let end = find_statement_end(sql, whole.end());
            let raw = &sql[start..end.min(sql.len())];
            let target = match kind {
                ObjectKind::Trigger | ObjectKind::Index | ObjectKind::Policy => on_re
                    .captures(raw)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str().to_string()),
                ObjectKind::Partition => partition_re.captures(raw).and_then(|c| c.get(1)).map(|m|m.as_str().to_string()),
                _ => None,
            };
            let mut name = cap.get(1)?.as_str().to_string();
            if kind == ObjectKind::Policy {
                if let Some(t) = &target {
                    name = format!("{} @ {}", name, t);
                }
            }
            Some(StatementMatch {
                kind,
                name,
                target,
                start,
                end,
            })
        })
        .collect()
}


fn extract_generated(constraints: &str) -> Option<(String,String)> {
    let re = Regex::new(r"(?is)\bGENERATED\s+ALWAYS\s+AS\s*\(").unwrap();
    let m = re.find(constraints)?;
    let open = constraints[m.start()..].find('(')? + m.start();
    let close = find_matching_paren(constraints, open)?;
    let expr = constraints[open+1..close].trim().to_owned();
    let tail = constraints[close+1..].to_ascii_uppercase();
    let kind = if tail.contains("STORED") { "stored" } else if tail.contains("VIRTUAL") { "virtual" } else { "generated" };
    Some((expr,kind.to_owned()))
}

fn parse_checks(item:&str,column:Option<&str>,source_line:usize)->Vec<CheckConstraint>{
    let re=Regex::new(r#"(?is)(?:\bCONSTRAINT\s+([A-Za-z_][A-Za-z0-9_$\"]*)\s+)?\bCHECK\s*\("#).unwrap();
    let mut out=Vec::new();let mut offset=0usize;
    while offset<item.len(){let Some(c)=re.captures(&item[offset..])else{break};let Some(w)=c.get(0)else{break};let start=offset+w.start();let open=item[start..].find('(').map(|x|x+start).unwrap_or(start);let Some(close)=find_matching_paren(item,open)else{break};out.push(CheckConstraint{name:c.get(1).map(|m|m.as_str().trim_matches('"').to_owned()),expression:item[open+1..close].trim().to_owned(),column:column.map(str::to_owned),source_line});offset=close+1;}
    out
}

fn parse_partitioning(raw:&str)->Option<Partitioning>{
    let re=Regex::new(r"(?is)\bPARTITION\s+BY\s+(RANGE|LIST|HASH)\s*\(").unwrap();
    let c=re.captures(raw)?;let w=c.get(0)?;let open=raw[w.start()..].find('(')?+w.start();let close=find_matching_paren(raw,open)?;
    Some(Partitioning{kind:c.get(1).map(|m|m.as_str().to_ascii_lowercase()).unwrap_or_else(||"unknown".into()),expression:Some(raw[open+1..close].trim().to_owned()),parent:None,bound:None})
}

fn migration_for_line(migrations: &[Migration], line: usize) -> Option<u32> {
    migrations
        .iter()
        .rev()
        .find(|m| m.line <= line)
        .map(|m| m.version)
}

fn find_constraint_keyword(rest: &str) -> Option<usize> {
    let re = Regex::new(
        r"(?i)\s+(NOT\s+NULL|NULL|DEFAULT|PRIMARY\s+KEY|REFERENCES|CHECK|UNIQUE|COLLATE|GENERATED|CONSTRAINT|AS|IDENTITY)\b",
    )
    .unwrap();
    re.find(rest).map(|m| m.start())
}

fn extract_default(constraints: &str) -> Option<String> {
    let upper = constraints.to_ascii_uppercase();
    let start = upper.find("DEFAULT")? + "DEFAULT".len();
    let after = &constraints[start..];
    let end = find_constraint_keyword(after).unwrap_or(after.len());
    let expr = after[..end].trim();
    (!expr.is_empty()).then(|| expr.to_string())
}

fn parse_on_delete(tail: &str) -> Option<String> {
    let re = Regex::new(r"(?i)\bON\s+DELETE\s+(CASCADE|RESTRICT|SET\s+NULL|SET\s+DEFAULT|NO\s+ACTION)").unwrap();
    re.captures(tail)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_ascii_uppercase())
}

fn parse_ident_list(list: &str) -> Vec<String> {
    split_top_level(list, ',')
        .into_iter()
        .map(|v| v.trim().trim_matches('"').to_string())
        .filter(|v| !v.is_empty())
        .collect()
}

fn take_identifier(s: &str) -> Option<(String, &str)> {
    let s = s.trim_start();
    if let Some(stripped) = s.strip_prefix('"') {
        let mut escaped = false;
        for (i, ch) in stripped.char_indices() {
            if ch == '"' {
                if escaped {
                    escaped = false;
                    continue;
                }
                let name = stripped[..i].replace("\"\"", "\"");
                let rest = &stripped[i + 1..];
                return Some((name, rest));
            }
            escaped = ch == '"';
        }
        None
    } else {
        let end = s
            .char_indices()
            .find(|(_, c)| c.is_whitespace())
            .map(|(i, _)| i)
            .unwrap_or(s.len());
        if end == 0 {
            None
        } else {
            Some((s[..end].trim_matches(',').to_string(), &s[end..]))
        }
    }
}

fn split_top_level(s: &str, sep: char) -> Vec<&str> {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut depth = 0_i32;
    let mut start = 0_usize;
    let mut i = 0_usize;
    let mut single = false;
    let mut double = false;
    let mut dollar: Option<String> = None;

    while i < bytes.len() {
        if let Some(tag) = &dollar {
            if bytes[i..].starts_with(tag.as_bytes()) {
                i += tag.len();
                dollar = None;
                continue;
            }
            i += 1;
            continue;
        }
        let ch = bytes[i] as char;
        if single {
            if ch == '\'' {
                if i + 1 < bytes.len() && bytes[i + 1] as char == '\'' {
                    i += 2;
                    continue;
                }
                single = false;
            }
            i += 1;
            continue;
        }
        if double {
            if ch == '"' {
                if i + 1 < bytes.len() && bytes[i + 1] as char == '"' {
                    i += 2;
                    continue;
                }
                double = false;
            }
            i += 1;
            continue;
        }
        if ch == '\'' {
            single = true;
            i += 1;
            continue;
        }
        if ch == '"' {
            double = true;
            i += 1;
            continue;
        }
        if ch == '$' {
            if let Some(tag) = dollar_tag_at(s, i) {
                i += tag.len();
                dollar = Some(tag);
                continue;
            }
        }
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            _ if ch == sep && depth == 0 => {
                out.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    out.push(&s[start..]);
    out
}

fn find_matching_paren(sql: &str, open: usize) -> Option<usize> {
    let bytes = sql.as_bytes();
    if bytes.get(open).copied()? as char != '(' {
        return None;
    }
    let mut depth = 0_i32;
    let mut i = open;
    let mut single = false;
    let mut double = false;
    let mut dollar: Option<String> = None;
    let mut line_comment = false;
    let mut block_comment = 0_i32;

    while i < bytes.len() {
        if line_comment {
            if bytes[i] as char == '\n' {
                line_comment = false;
            }
            i += 1;
            continue;
        }
        if block_comment > 0 {
            if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                block_comment += 1;
                i += 2;
                continue;
            }
            if i + 1 < bytes.len() && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                block_comment -= 1;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if let Some(tag) = &dollar {
            if bytes[i..].starts_with(tag.as_bytes()) {
                i += tag.len();
                dollar = None;
                continue;
            }
            i += 1;
            continue;
        }
        let ch = bytes[i] as char;
        if single {
            if ch == '\'' {
                if i + 1 < bytes.len() && bytes[i + 1] as char == '\'' {
                    i += 2;
                    continue;
                }
                single = false;
            }
            i += 1;
            continue;
        }
        if double {
            if ch == '"' {
                if i + 1 < bytes.len() && bytes[i + 1] as char == '"' {
                    i += 2;
                    continue;
                }
                double = false;
            }
            i += 1;
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            line_comment = true;
            i += 2;
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            block_comment = 1;
            i += 2;
            continue;
        }
        if ch == '\'' {
            single = true;
            i += 1;
            continue;
        }
        if ch == '"' {
            double = true;
            i += 1;
            continue;
        }
        if ch == '$' {
            if let Some(tag) = dollar_tag_at(sql, i) {
                i += tag.len();
                dollar = Some(tag);
                continue;
            }
        }
        if ch == '(' {
            depth += 1;
        } else if ch == ')' {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

fn find_statement_end(sql: &str, start: usize) -> usize {
    let bytes = sql.as_bytes();
    let mut i = start.min(bytes.len());
    let mut single = false;
    let mut double = false;
    let mut dollar: Option<String> = None;
    let mut line_comment = false;
    let mut block_comment = 0_i32;

    while i < bytes.len() {
        if line_comment {
            if bytes[i] as char == '\n' {
                line_comment = false;
            }
            i += 1;
            continue;
        }
        if block_comment > 0 {
            if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                block_comment += 1;
                i += 2;
                continue;
            }
            if i + 1 < bytes.len() && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                block_comment -= 1;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if let Some(tag) = &dollar {
            if bytes[i..].starts_with(tag.as_bytes()) {
                i += tag.len();
                dollar = None;
                continue;
            }
            i += 1;
            continue;
        }
        let ch = bytes[i] as char;
        if single {
            if ch == '\'' {
                if i + 1 < bytes.len() && bytes[i + 1] as char == '\'' {
                    i += 2;
                    continue;
                }
                single = false;
            }
            i += 1;
            continue;
        }
        if double {
            if ch == '"' {
                if i + 1 < bytes.len() && bytes[i + 1] as char == '"' {
                    i += 2;
                    continue;
                }
                double = false;
            }
            i += 1;
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            line_comment = true;
            i += 2;
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            block_comment = 1;
            i += 2;
            continue;
        }
        if ch == '\'' {
            single = true;
            i += 1;
            continue;
        }
        if ch == '"' {
            double = true;
            i += 1;
            continue;
        }
        if ch == '$' {
            if let Some(tag) = dollar_tag_at(sql, i) {
                i += tag.len();
                dollar = Some(tag);
                continue;
            }
        }
        if ch == ';' {
            return i + 1;
        }
        i += 1;
    }
    bytes.len()
}

fn dollar_tag_at(s: &str, i: usize) -> Option<String> {
    let tail = s.get(i..)?;
    if !tail.starts_with('$') {
        return None;
    }
    let end = tail[1..].find('$')? + 1;
    let tag = &tail[..=end];
    let inner = &tag[1..tag.len() - 1];
    if inner.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        Some(tag.to_string())
    } else {
        None
    }
}

fn line_starts(sql: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (i, b) in sql.bytes().enumerate() {
        if b == b'\n' {
            starts.push(i + 1);
        }
    }
    starts
}

fn line_of(starts: &[usize], byte: usize) -> usize {
    match starts.binary_search(&byte) {
        Ok(idx) => idx + 1,
        Err(idx) => idx.max(1),
    }
}

fn byte_offset_for_line(starts: &[usize], line: usize) -> usize {
    if line == 0 {
        0
    } else {
        starts.get(line - 1).copied().unwrap_or_else(|| *starts.last().unwrap_or(&0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_table_and_fk() {
        let sql = r#"
-- MIGRATION 0001: core.sql
CREATE TABLE a (id uuid PRIMARY KEY, name text NOT NULL);
CREATE TABLE b (
  id uuid PRIMARY KEY,
  a_id uuid NOT NULL REFERENCES a(id) ON DELETE CASCADE
);
"#;
        let model = parse_sql("test.sql", sql.to_string());
        assert_eq!(model.tables.len(), 2);
        assert_eq!(model.relationships.len(), 1);
        assert_eq!(model.tables[1].columns.len(), 2);
        assert_eq!(model.tables[1].foreign_keys[0].to_table, "a");
    }

    #[test]
    fn statement_scanner_ignores_function_semicolons() {
        let sql = "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN PERFORM 1; END; $$;\nCREATE TABLE t(id int);";
        let funcs = find_named_statements(sql, ObjectKind::Function);
        assert_eq!(funcs.len(), 1);
        assert!(funcs[0].end < sql.len());
    }
}


#[derive(Default)]
pub struct PostgreSqlParser;

impl phx_sql_parser_api::SqlDialectParser for PostgreSqlParser {
    fn dialect(&self) -> phx_sql_parser_api::SqlDialect {
        phx_sql_parser_api::SqlDialect::PostgreSql
    }

    fn parse(&self, source_name: &str, sql: &str) -> Result<phx_sql_model::SqlModel, phx_sql_parser_api::ParseError> {
        Ok(parse_sql(source_name.to_owned(), sql.to_owned()))
    }
}
