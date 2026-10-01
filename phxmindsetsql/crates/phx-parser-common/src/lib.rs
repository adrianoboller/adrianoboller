use phx_sql_model::{
    canonical_name, CheckConstraint, Column, ForeignKey, Migration, ObjectKind, Partitioning,
    SqlModel, SqlObject, SqlStats, Table,
};
use regex::Regex;
use std::collections::{BTreeMap, HashMap, HashSet};

const IDENT_PART: &str = r#"(?:\[[^\]]+\]|`[^`]+`|"(?:[^"]|"")+"|[A-Za-z_][A-Za-z0-9_$#@]*)"#;
pub fn identifier_pattern() -> String { format!(r#"{0}(?:\s*\.\s*{0})*"#, IDENT_PART) }

#[derive(Clone, Debug)]
pub struct ObjectSpec { pub kind: ObjectKind, pub pattern: String, pub target_pattern: Option<String> }
#[derive(Clone, Debug)]
pub struct GenericDialectConfig {
    pub dialect: &'static str,
    pub create_table_pattern: String,
    pub alter_fk_pattern: Option<String>,
    pub constraint_keywords: Vec<&'static str>,
    pub object_specs: Vec<ObjectSpec>,
    pub go_batch: bool,
}

pub fn parse_generic(source_name: &str, sql: &str, cfg: &GenericDialectConfig) -> Result<SqlModel, String> {
    let starts = line_starts(sql);
    let migrations = parse_migrations(sql, &starts);
    let mut tables = parse_tables(sql, &starts, &migrations, cfg)?;
    let mut relationships: Vec<ForeignKey> = tables.iter().flat_map(|t| t.foreign_keys.clone()).collect();
    if let Some(pattern) = &cfg.alter_fk_pattern {
        let mut sigs: HashSet<String> = relationships.iter().map(ForeignKey::signature).collect();
        for fk in parse_alter_fks(sql, &starts, pattern)? { if sigs.insert(fk.signature()) { relationships.push(fk); } }
    }
    let lookup: HashMap<String, usize> = tables.iter().enumerate().map(|(i,t)|(canonical_name(&t.name),i)).collect();
    for fk in &relationships {
        if let Some(&i) = lookup.get(&canonical_name(&fk.from_table)) {
            if !tables[i].foreign_keys.iter().any(|x| x.signature() == fk.signature()) { tables[i].foreign_keys.push(fk.clone()); }
        }
    }
    let objects = collect_objects(sql, &starts, &migrations, &tables, cfg)?;
    let stats = SqlStats {
        lines: sql.lines().count(), bytes: sql.len(), tables: tables.len(), relationships: relationships.len(),
        columns: tables.iter().map(|t|t.columns.len()).sum(),
        check_constraints: tables.iter().map(|t|t.check_constraints.len()).sum(),
        generated_columns: tables.iter().flat_map(|t|t.columns.iter()).filter(|c|c.generated_expression.is_some()).count(),
        views: objects.iter().filter(|o|o.kind==ObjectKind::View).count(),
        materialized_views: objects.iter().filter(|o|o.kind==ObjectKind::MaterializedView).count(),
        sequences: objects.iter().filter(|o|o.kind==ObjectKind::Sequence).count(),
        functions: objects.iter().filter(|o|o.kind==ObjectKind::Function).count(),
        triggers: objects.iter().filter(|o|o.kind==ObjectKind::Trigger).count(),
        indexes: objects.iter().filter(|o|o.kind==ObjectKind::Index).count(),
        policies: objects.iter().filter(|o|o.kind==ObjectKind::Policy).count(),
        schemas: objects.iter().filter(|o|o.kind==ObjectKind::Schema).count(),
        synonyms: objects.iter().filter(|o|o.kind==ObjectKind::Synonym).count(),
        events: objects.iter().filter(|o|o.kind==ObjectKind::Event).count(),
        extensions: objects.iter().filter(|o|o.kind==ObjectKind::Extension).count(),
        partitions: objects.iter().filter(|o|o.kind==ObjectKind::Partition).count() + tables.iter().filter(|t|t.partitioning.is_some()).count(),
        migrations: migrations.len(),
    };
    Ok(SqlModel { source_name:source_name.to_owned(), sql:sql.to_owned(), migrations, tables, relationships, objects, stats })
}

fn parse_migrations(sql:&str,starts:&[usize])->Vec<Migration>{
    let re=Regex::new(r"(?im)^--\s*MIGRATION\s+(\d+):\s*([^\r\n]+)").unwrap();
    re.captures_iter(sql).filter_map(|c|{let w=c.get(0)?;Some(Migration{version:c.get(1)?.as_str().parse().ok()?,name:c.get(2)?.as_str().trim().to_owned(),line:line_of(starts,w.start())})}).collect()
}

fn parse_tables(sql:&str,starts:&[usize],migrations:&[Migration],cfg:&GenericDialectConfig)->Result<Vec<Table>,String>{
    let re=Regex::new(&cfg.create_table_pattern).map_err(|e|e.to_string())?;
    let pk_re=Regex::new(r"(?is)(?:^|\s)(?:CONSTRAINT\s+\S+\s+)?PRIMARY\s+KEY(?:\s+(?:CLUSTERED|NONCLUSTERED))?\s*\(([^)]*)\)").unwrap();
    let id=identifier_pattern();
    let fk_re=Regex::new(&format!(r"(?is)^(?:CONSTRAINT\s+({id})\s+)?FOREIGN\s+KEY\s*\(([^)]*)\)\s+REFERENCES\s+({id})\s*\(([^)]*)\)(.*)$")).map_err(|e|e.to_string())?;
    let inline_re=Regex::new(&format!(r"(?is)\bREFERENCES\s+({id})\s*\(([^)]*)\)(.*)$")).map_err(|e|e.to_string())?;
    let constraint_re=constraint_regex(&cfg.constraint_keywords)?;
    let mut out=Vec::new();
    for cap in re.captures_iter(sql){
        let Some(w)=cap.get(0)else{continue};
        let name=clean_identifier(cap.name("name").or_else(||cap.get(1)).map(|m|m.as_str()).unwrap_or(""));
        let open=w.end().saturating_sub(1); if sql.as_bytes().get(open).copied()!=Some(b'('){continue}
        let close=find_matching_paren(sql,open).unwrap_or(sql.len().saturating_sub(1));
        let end=find_statement_end(sql,close+1,cfg.go_batch); let line=line_of(starts,w.start()); let body=&sql[open+1..close];
        let mut columns=Vec::new(); let mut pks=Vec::new(); let mut fks=Vec::new(); let mut checks=Vec::new();
        for raw_item in split_top(body,','){
            let item=raw_item.trim(); if item.is_empty(){continue}
            if let Some(c)=pk_re.captures(item){if let Some(x)=c.get(1){for col in parse_ident_list(x.as_str()){if !pks.iter().any(|p|canonical_name(p)==canonical_name(&col)){pks.push(col)}}}}
            if let Some(c)=fk_re.captures(item){fks.push(ForeignKey{constraint_name:c.get(1).map(|m|clean_identifier(m.as_str())),from_table:name.clone(),from_columns:c.get(2).map(|m|parse_ident_list(m.as_str())).unwrap_or_default(),to_table:c.get(3).map(|m|clean_identifier(m.as_str())).unwrap_or_default(),to_columns:c.get(4).map(|m|parse_ident_list(m.as_str())).unwrap_or_default(),on_delete:c.get(5).and_then(|m|parse_on_delete(m.as_str())),source_line:line});continue}
            let upper=item.to_ascii_uppercase();
            if upper.starts_with("CHECK") || (upper.starts_with("CONSTRAINT ") && upper.contains(" CHECK")) { checks.extend(parse_checks(item,None,line)); continue; }
            if upper.starts_with("CONSTRAINT ")||upper.starts_with("PRIMARY KEY")||upper.starts_with("FOREIGN KEY")||upper.starts_with("UNIQUE"){continue}
            let Some((cn,rest0))=take_identifier(item)else{continue}; let rest=rest0.trim(); if rest.is_empty(){continue}
            let cut=constraint_re.find(rest).map(|m|m.start()).unwrap_or(rest.len());
            let mut dtype=rest[..cut].trim().to_owned(); let mut cons=&rest[cut..];
            if dtype.is_empty() && cfg.dialect=="sqlserver" && rest.to_ascii_uppercase().starts_with("AS") { dtype="computed".into(); cons=rest; }
            if dtype.is_empty(){continue}
            let uc=cons.to_ascii_uppercase(); let is_pk=uc.contains("PRIMARY KEY"); let generated=extract_generated(cons,cfg.dialect);
            let mut col=Column{name:cn.clone(),data_type:dtype,nullable:!uc.contains("NOT NULL")&&!is_pk,default_expr:extract_default(cons,&cfg.constraint_keywords),primary_key:is_pk,unique:uc.contains("UNIQUE"),generated_expression:generated.as_ref().map(|x|x.0.clone()),generated_kind:generated.map(|x|x.1)};
            checks.extend(parse_checks(cons,Some(&cn),line));
            if is_pk&&!pks.iter().any(|p|canonical_name(p)==canonical_name(&cn)){pks.push(cn.clone())}
            if let Some(c)=inline_re.captures(cons){fks.push(ForeignKey{constraint_name:None,from_table:name.clone(),from_columns:vec![cn.clone()],to_table:c.get(1).map(|m|clean_identifier(m.as_str())).unwrap_or_default(),to_columns:c.get(2).map(|m|parse_ident_list(m.as_str())).unwrap_or_default(),on_delete:c.get(3).and_then(|m|parse_on_delete(m.as_str())),source_line:line})}
            if pks.iter().any(|p|canonical_name(p)==canonical_name(&col.name)){col.primary_key=true;col.nullable=false} columns.push(col)
        }
        for c in &mut columns{if pks.iter().any(|p|canonical_name(p)==canonical_name(&c.name)){c.primary_key=true;c.nullable=false}}
        let raw_sql=sql[w.start()..end.min(sql.len())].trim().to_owned();
        out.push(Table{name,line,end_line:line_of(starts,end.saturating_sub(1)),migration:migration_for_line(migrations,line),columns,primary_key:pks,foreign_keys:fks,check_constraints:checks,partitioning:parse_partitioning(&raw_sql,cfg.dialect),statistics:None,raw_sql});
    }
    Ok(out)
}

fn extract_generated(cons:&str,dialect:&str)->Option<(String,String)>{
    let upper=cons.to_ascii_uppercase();
    let pos=if let Some(p)=upper.find("GENERATED ALWAYS AS"){p+"GENERATED ALWAYS AS".len()}else if dialect=="sqlserver"{upper.find("AS").map(|p|p+2)?}else{return None};
    let tail=&cons[pos..]; let rel=tail.find('(')?; let open=pos+rel; let close=find_matching_paren(cons,open)?; let expr=cons[open+1..close].trim().to_owned(); if expr.is_empty(){return None}
    let after=cons[close+1..].to_ascii_uppercase(); let kind=if dialect=="sqlserver"{if after.contains("PERSISTED"){"persisted"}else{"virtual"}}else if after.contains("STORED"){"stored"}else if after.contains("VIRTUAL"){"virtual"}else{"generated"};
    Some((expr,kind.into()))
}

fn parse_checks(text:&str,column:Option<&str>,line:usize)->Vec<CheckConstraint>{
    let mut out=Vec::new(); let upper=text.to_ascii_uppercase(); let bytes=upper.as_bytes(); let mut at=0usize;
    while at<bytes.len(){let Some(rel)=upper[at..].find("CHECK")else{break};let p=at+rel+5;let Some(o)=text[p..].find('(')else{break};let open=p+o;let Some(close)=find_matching_paren(text,open)else{break};let before=&text[..at+rel];let name=Regex::new(r"(?i)CONSTRAINT\s+([^\s]+)\s*$").ok().and_then(|r|r.captures(before).and_then(|c|c.get(1).map(|m|clean_identifier(m.as_str()))));out.push(CheckConstraint{name,expression:text[open+1..close].trim().to_owned(),column:column.map(str::to_owned),source_line:line});at=close+1;}
    out
}

fn parse_partitioning(raw:&str,dialect:&str)->Option<Partitioning>{
    match dialect{
        "mysql"=>{let r=Regex::new(r"(?is)\bPARTITION\s+BY\s+(.+?)(?:;|$)").ok()?;let c=r.captures(raw)?;Some(Partitioning{kind:"mysql".into(),expression:Some(c.get(1)?.as_str().trim().to_owned()),parent:None,bound:None})},
        "sqlserver"=>{let r=Regex::new(r"(?is)\)\s+ON\s+([^\s(;]+)\s*\(([^)]*)\)\s*;?\s*$").ok()?;let c=r.captures(raw)?;Some(Partitioning{kind:"partition_scheme".into(),expression:Some(format!("{}({})",c.get(1)?.as_str(),c.get(2)?.as_str())),parent:None,bound:None})},
        _=>None
    }
}

fn parse_alter_fks(sql:&str,starts:&[usize],pattern:&str)->Result<Vec<ForeignKey>,String>{let re=Regex::new(pattern).map_err(|e|e.to_string())?;Ok(re.captures_iter(sql).filter_map(|c|{let w=c.get(0)?;let from=c.name("from").or_else(||c.get(1))?;let fc=c.name("from_cols").or_else(||c.get(3))?;let to=c.name("to").or_else(||c.get(4))?;let tc=c.name("to_cols").or_else(||c.get(5))?;Some(ForeignKey{constraint_name:c.name("constraint").or_else(||c.get(2)).map(|m|clean_identifier(m.as_str())),from_table:clean_identifier(from.as_str()),from_columns:parse_ident_list(fc.as_str()),to_table:clean_identifier(to.as_str()),to_columns:parse_ident_list(tc.as_str()),on_delete:c.name("tail").or_else(||c.get(6)).and_then(|m|parse_on_delete(m.as_str())),source_line:line_of(starts,w.start())})}).collect())}

fn collect_objects(sql:&str,starts:&[usize],migrations:&[Migration],tables:&[Table],cfg:&GenericDialectConfig)->Result<Vec<SqlObject>,String>{
    let mut ms:Vec<(ObjectKind,String,Option<String>,usize,usize)>=Vec::new();for t in tables{let st=byte_for_line(starts,t.line),en=byte_for_line(starts,t.end_line+1).min(sql.len());ms.push((ObjectKind::Table,t.name.clone(),None,st,en))}
    for spec in &cfg.object_specs{let re=Regex::new(&spec.pattern).map_err(|e|e.to_string())?;let tr=match &spec.target_pattern{Some(p)=>Some(Regex::new(p).map_err(|e|e.to_string())?),None=>None};for c in re.captures_iter(sql){let Some(w)=c.get(0)else{continue};let Some(n)=c.name("name").or_else(||c.get(1))else{continue};let st=w.start(),en=find_statement_end(sql,w.end(),cfg.go_batch),raw=&sql[st..en.min(sql.len())];let target=tr.as_ref().and_then(|r|r.captures(raw).and_then(|x|x.name("target").or_else(||x.get(1)).map(|m|clean_identifier(m.as_str()))));ms.push((spec.kind,clean_identifier(n.as_str()),target,st,en))}}
    ms.sort_by_key(|x|x.3);Ok(ms.into_iter().enumerate().map(|(id,(kind,name,target,st,en))|{let line=line_of(starts,st);let raw=sql[st..en.min(sql.len())].trim().to_owned();let mut metadata=BTreeMap::new();if kind==ObjectKind::Event{if let Some(c)=Regex::new(r"(?is)\bON\s+SCHEDULE\s+(.+?)(?=\bDO\b|$)").unwrap().captures(&raw){if let Some(x)=c.get(1){metadata.insert("schedule".into(),x.as_str().trim().into());}}}SqlObject{id,kind,name,line,end_line:line_of(starts,en.saturating_sub(1)),migration:migration_for_line(migrations,line),target,raw_sql:raw,metadata}}).collect())
}

fn constraint_regex(keys:&[&str])->Result<Regex,String>{let a=keys.iter().map(|k|k.split_whitespace().map(regex::escape).collect::<Vec<_>>().join(r"\s+")).collect::<Vec<_>>().join("|");Regex::new(&format!(r"(?i)\s+(?:{a})\b|^(?:{a})\b")).map_err(|e|e.to_string())}
fn extract_default(cons:&str,keys:&[&str])->Option<String>{let h=Regex::new(r"(?i)\bDEFAULT\b").unwrap().find(cons)?;let after=&cons[h.end()..];let f=keys.iter().copied().filter(|k|!k.eq_ignore_ascii_case("DEFAULT")).collect::<Vec<_>>();let e=constraint_regex(&f).ok().and_then(|r|r.find(after).map(|m|m.start())).unwrap_or(after.len());let v=after[..e].trim();(!v.is_empty()).then(||v.to_owned())}
fn parse_on_delete(t:&str)->Option<String>{let r=Regex::new(r"(?i)\bON\s+DELETE\s+(CASCADE|RESTRICT|SET\s+NULL|SET\s+DEFAULT|NO\s+ACTION)\b").unwrap();r.captures(t).and_then(|c|c.get(1)).map(|m|m.as_str().split_whitespace().collect::<Vec<_>>().join(" ").to_ascii_uppercase())}
fn parse_ident_list(s:&str)->Vec<String>{split_top(s,',').into_iter().map(|x|{let mut v=x.trim().to_owned();for suf in [" ASC"," DESC"]{if v.to_ascii_uppercase().ends_with(suf){v.truncate(v.len()-suf.len());break}}clean_identifier(&v)}).filter(|x|!x.is_empty()).collect()}
pub fn clean_identifier(s:&str)->String{s.split('.').map(|p|{let p=p.trim();if p.starts_with('[')&&p.ends_with(']'){p[1..p.len()-1].replace("]]","]")}else if p.starts_with('`')&&p.ends_with('`'){p[1..p.len()-1].replace("``","`")}else if p.starts_with('"')&&p.ends_with('"'){p[1..p.len()-1].replace("\"\"","\"")}else{p.to_owned()}}).collect::<Vec<_>>().join(".")}
fn take_identifier(s:&str)->Option<(String,&str)>{let s=s.trim_start();let f=s.chars().next()?;let end=match f{'['=>s.find(']')?+1,'`'=>s[1..].find('`')?+2,'"'=>s[1..].find('"')?+2,_=>s.char_indices().find(|(_,c)|c.is_whitespace()).map(|(i,_)|i).unwrap_or(s.len())};Some((clean_identifier(&s[..end]),&s[end..]))}
fn split_top(s:&str,sep:char)->Vec<&str>{let b=s.as_bytes();let mut o=Vec::new();let(mut st,mut d,mut i)=(0usize,0i32,0usize);let(mut sq,mut dq,mut bt,mut br)=(false,false,false,false);while i<b.len(){let c=b[i]as char,n=b.get(i+1).copied().map(char::from);if sq{if c=='\''{if n==Some('\''){i+=2;continue}sq=false}i+=1;continue}if dq{if c=='"'{if n==Some('"'){i+=2;continue}dq=false}i+=1;continue}if bt{if c=='`'{bt=false}i+=1;continue}if br{if c==']'{br=false}i+=1;continue}match c{'\''=>sq=true,'"'=>dq=true,'`'=>bt=true,'['=>br=true,'('=>d+=1,')'=>d=(d-1).max(0),_ if c==sep&&d==0=>{o.push(&s[st..i]);st=i+1},_=>{}}i+=1}o.push(&s[st..]);o}
fn find_matching_paren(s:&str,open:usize)->Option<usize>{let b=s.as_bytes();let(mut d,mut i)=(0i32,open);let(mut sq,mut dq,mut bt,mut br)=(false,false,false,false);while i<b.len(){let c=b[i]as char,n=b.get(i+1).copied().map(char::from);if sq{if c=='\''{if n==Some('\''){i+=2;continue}sq=false}i+=1;continue}if dq{if c=='"'{if n==Some('"'){i+=2;continue}dq=false}i+=1;continue}if bt{if c=='`'{bt=false}i+=1;continue}if br{if c==']'{br=false}i+=1;continue}match c{'\''=>sq=true,'"'=>dq=true,'`'=>bt=true,'['=>br=true,'('=>d+=1,')'=>{d-=1;if d==0{return Some(i)}},_=>{}}i+=1}None}
fn find_statement_end(s:&str,from:usize,go:bool)->usize{let b=s.as_bytes();let(mut i,mut d)=(from.min(b.len()),0i32);let(mut sq,mut dq,mut bt,mut br)=(false,false,false,false);let go_re=Regex::new(r"(?i)^\s*GO\s*(?:\r?\n|$)").unwrap();while i<b.len(){let c=b[i]as char,n=b.get(i+1).copied().map(char::from);if sq{if c=='\''{if n==Some('\''){i+=2;continue}sq=false}i+=1;continue}if dq{if c=='"'{if n==Some('"'){i+=2;continue}dq=false}i+=1;continue}if bt{if c=='`'{bt=false}i+=1;continue}if br{if c==']'{br=false}i+=1;continue}match c{'\''=>sq=true,'"'=>dq=true,'`'=>bt=true,'['=>br=true,'('=>d+=1,')'=>d=(d-1).max(0),';' if d==0=>return i+1,'\n' if go&&d==0=>{if go_re.is_match(&s[i+1..]){return i+1}},_=>{}}i+=1}s.len()}
fn migration_for_line(ms:&[Migration],line:usize)->Option<u32>{ms.iter().rev().find(|m|m.line<=line).map(|m|m.version)}
fn line_starts(s:&str)->Vec<usize>{let mut o=vec![0];for(i,b)in s.bytes().enumerate(){if b==b'\n'{o.push(i+1)}}o}
fn line_of(starts:&[usize],off:usize)->usize{match starts.binary_search(&off){Ok(i)=>i+1,Err(i)=>i.max(1)}}
fn byte_for_line(starts:&[usize],line:usize)->usize{starts.get(line.saturating_sub(1)).copied().unwrap_or_else(||*starts.last().unwrap_or(&0))}
