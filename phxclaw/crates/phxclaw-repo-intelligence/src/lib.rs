#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};
use tree_sitter::{Language, Node, Parser, Point};
use walkdir::WalkDir;

const MAX_PARSE_FILE_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoFile {
    pub path: String,
    pub language: String,
    pub bytes: u64,
    pub lines: usize,
    pub sha256: String,
    pub score: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInventory {
    pub files: Vec<RepoFile>,
    pub languages: BTreeMap<String, usize>,
    pub total_bytes: u64,
    pub total_lines: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParseStatus {
    Parsed,
    ParsedWithErrors,
    UnsupportedLanguage,
    Oversize,
    ReadError,
    ParserError,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileParse {
    pub path: String,
    pub language: String,
    pub status: ParseStatus,
    pub root_kind: Option<String>,
    pub parse_error_nodes: usize,
    pub named_nodes: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRange {
    pub start_row: usize,
    pub start_column: usize,
    pub end_row: usize,
    pub end_column: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSymbol {
    pub symbol_id: String,
    pub file_path: String,
    pub language: String,
    pub kind: String,
    pub name: String,
    pub qualified_hint: String,
    pub range: SourceRange,
    pub complexity: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoCall {
    pub call_id: String,
    pub file_path: String,
    pub caller_symbol_id: Option<String>,
    pub target_text: String,
    pub target_name: String,
    pub resolved_symbol_ids: Vec<String>,
    pub range: SourceRange,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoDependency {
    pub dependency_id: String,
    pub file_path: String,
    pub kind: String,
    pub raw: String,
    pub resolved_file: Option<String>,
    pub range: SourceRange,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoHotspot {
    pub file_path: String,
    pub pagerank_ppm: u32,
    pub inbound_edges: usize,
    pub outbound_edges: usize,
    pub symbol_count: usize,
    pub complexity: u64,
    pub hotspot_score: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoIntelligence {
    pub inventory: RepoInventory,
    pub parses: Vec<FileParse>,
    pub symbols: Vec<RepoSymbol>,
    pub calls: Vec<RepoCall>,
    pub dependencies: Vec<RepoDependency>,
    pub hotspots: Vec<RepoHotspot>,
}

pub fn scan(root: &Path) -> RepoInventory {
    let mut files = Vec::new();
    let mut languages = BTreeMap::new();
    let mut total_bytes = 0;
    let mut total_lines = 0;
    for e in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !e.file_type().is_file() || ignored(e.path(), root) {
            continue;
        }
        let Ok(bytes) = fs::read(e.path()) else {
            continue;
        };
        let language = detect(e.path());
        if language == "binary" {
            continue;
        }
        let lines = String::from_utf8_lossy(&bytes).lines().count();
        let rel = relative(root, e.path());
        let score = importance_score(&rel, lines, bytes.len() as u64);
        *languages.entry(language.to_string()).or_insert(0) += 1;
        total_bytes += bytes.len() as u64;
        total_lines += lines;
        files.push(RepoFile {
            path: rel,
            language: language.into(),
            bytes: bytes.len() as u64,
            lines,
            sha256: sha256_hex(&bytes),
            score,
        });
    }
    files.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.path.cmp(&b.path)));
    RepoInventory {
        files,
        languages,
        total_bytes,
        total_lines,
    }
}

pub fn analyze(root: &Path) -> RepoIntelligence {
    let inventory = scan(root);
    let mut parses = Vec::new();
    let mut symbols = Vec::new();
    let mut calls = Vec::new();
    let mut dependencies = Vec::new();
    for file in &inventory.files {
        let path = root.join(&file.path);
        if file.bytes > MAX_PARSE_FILE_BYTES {
            parses.push(FileParse {
                path: file.path.clone(),
                language: file.language.clone(),
                status: ParseStatus::Oversize,
                root_kind: None,
                parse_error_nodes: 0,
                named_nodes: 0,
            });
            continue;
        }
        let Some(language) = language_for(&path, &file.language) else {
            parses.push(FileParse {
                path: file.path.clone(),
                language: file.language.clone(),
                status: ParseStatus::UnsupportedLanguage,
                root_kind: None,
                parse_error_nodes: 0,
                named_nodes: 0,
            });
            continue;
        };
        let Ok(bytes) = fs::read(&path) else {
            parses.push(FileParse {
                path: file.path.clone(),
                language: file.language.clone(),
                status: ParseStatus::ReadError,
                root_kind: None,
                parse_error_nodes: 0,
                named_nodes: 0,
            });
            continue;
        };
        let mut parser = Parser::new();
        if parser.set_language(&language).is_err() {
            parses.push(FileParse {
                path: file.path.clone(),
                language: file.language.clone(),
                status: ParseStatus::ParserError,
                root_kind: None,
                parse_error_nodes: 0,
                named_nodes: 0,
            });
            continue;
        }
        let Some(tree) = parser.parse(&bytes, None) else {
            parses.push(FileParse {
                path: file.path.clone(),
                language: file.language.clone(),
                status: ParseStatus::ParserError,
                root_kind: None,
                parse_error_nodes: 0,
                named_nodes: 0,
            });
            continue;
        };
        let root_node = tree.root_node();
        let mut stats = (0usize, 0usize);
        walk_stats(root_node, &mut stats);
        let status = if root_node.has_error() || stats.0 > 0 {
            ParseStatus::ParsedWithErrors
        } else {
            ParseStatus::Parsed
        };
        parses.push(FileParse {
            path: file.path.clone(),
            language: file.language.clone(),
            status,
            root_kind: Some(root_node.kind().to_string()),
            parse_error_nodes: stats.0,
            named_nodes: stats.1,
        });
        let mut stack = Vec::<String>::new();
        visit(
            root_node,
            &bytes,
            &file.path,
            &file.language,
            &mut stack,
            &mut symbols,
            &mut calls,
            &mut dependencies,
        );
    }
    resolve_calls(&mut calls, &symbols);
    resolve_dependencies(&mut dependencies, &inventory.files);
    let hotspots = compute_hotspots(&inventory, &symbols, &calls, &dependencies);
    RepoIntelligence {
        inventory,
        parses,
        symbols,
        calls,
        dependencies,
        hotspots,
    }
}

fn visit(
    node: Node<'_>,
    src: &[u8],
    file: &str,
    language: &str,
    stack: &mut Vec<String>,
    symbols: &mut Vec<RepoSymbol>,
    calls: &mut Vec<RepoCall>,
    deps: &mut Vec<RepoDependency>,
) {
    let kind = node.kind();
    let mut pushed = false;
    if is_symbol_kind(language, kind) {
        if let Some(name) = symbol_name(node, src) {
            let qualified = if stack.is_empty() {
                name.clone()
            } else {
                format!("{}::{}", stack.join("::"), name)
            };
            let id = stable_id(&[
                "symbol",
                file,
                kind,
                &qualified,
                &node.start_byte().to_string(),
                &node.end_byte().to_string(),
            ]);
            symbols.push(RepoSymbol {
                symbol_id: id,
                file_path: file.into(),
                language: language.into(),
                kind: kind.into(),
                name: name.clone(),
                qualified_hint: qualified,
                range: range(node),
                complexity: branch_complexity(node),
            });
            if is_scope_kind(language, kind) {
                stack.push(name);
                pushed = true;
            }
        }
    }
    if is_call_kind(language, kind) {
        let text = call_target(node, src).unwrap_or_else(|| bounded_text(node, src, 240));
        let name = normalize_target_name(&text);
        let caller = find_enclosing_symbol(file, node.start_position(), symbols);
        calls.push(RepoCall {
            call_id: stable_id(&[
                "call",
                file,
                &node.start_byte().to_string(),
                &node.end_byte().to_string(),
                &text,
            ]),
            file_path: file.into(),
            caller_symbol_id: caller,
            target_text: text,
            target_name: name,
            resolved_symbol_ids: vec![],
            range: range(node),
        });
    }
    if is_dependency_kind(language, kind) {
        let raw = bounded_text(node, src, 500);
        deps.push(RepoDependency {
            dependency_id: stable_id(&["dep", file, kind, &raw, &node.start_byte().to_string()]),
            file_path: file.into(),
            kind: kind.into(),
            raw,
            resolved_file: None,
            range: range(node),
        });
    }
    let mut c = node.walk();
    for child in node.children(&mut c) {
        visit(child, src, file, language, stack, symbols, calls, deps)
    }
    if pushed {
        stack.pop();
    }
}

fn language_for(path: &Path, language: &str) -> Option<Language> {
    match language {
        "rust" => Some(tree_sitter_rust::LANGUAGE.into()),
        "python" => Some(tree_sitter_python::LANGUAGE.into()),
        "javascript" => Some(tree_sitter_javascript::LANGUAGE.into()),
        "typescript" => {
            if path
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.eq_ignore_ascii_case("tsx"))
            {
                Some(tree_sitter_typescript::LANGUAGE_TSX.into())
            } else {
                Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
            }
        }
        "go" => Some(tree_sitter_go::LANGUAGE.into()),
        "c" => Some(tree_sitter_c::LANGUAGE.into()),
        "cpp" => Some(tree_sitter_cpp::LANGUAGE.into()),
        "java" => Some(tree_sitter_java::LANGUAGE.into()),
        _ => None,
    }
}
fn is_symbol_kind(lang: &str, k: &str) -> bool {
    match lang {
        "rust" => matches!(
            k,
            "function_item"
                | "struct_item"
                | "enum_item"
                | "trait_item"
                | "impl_item"
                | "mod_item"
                | "type_item"
                | "const_item"
                | "static_item"
        ),
        "python" => matches!(k, "function_definition" | "class_definition"),
        "javascript" | "typescript" => matches!(
            k,
            "function_declaration"
                | "class_declaration"
                | "method_definition"
                | "generator_function_declaration"
                | "interface_declaration"
                | "type_alias_declaration"
        ),
        "go" => matches!(
            k,
            "function_declaration" | "method_declaration" | "type_spec"
        ),
        "c" => matches!(
            k,
            "function_definition" | "struct_specifier" | "enum_specifier" | "type_definition"
        ),
        "cpp" => matches!(
            k,
            "function_definition"
                | "class_specifier"
                | "struct_specifier"
                | "enum_specifier"
                | "namespace_definition"
                | "type_definition"
        ),
        "java" => matches!(
            k,
            "class_declaration"
                | "interface_declaration"
                | "method_declaration"
                | "constructor_declaration"
                | "enum_declaration"
                | "record_declaration"
        ),
        _ => false,
    }
}
fn is_scope_kind(lang: &str, k: &str) -> bool {
    match lang {
        "rust" => matches!(
            k,
            "struct_item" | "enum_item" | "trait_item" | "impl_item" | "mod_item"
        ),
        "python" => k == "class_definition",
        "javascript" | "typescript" => matches!(k, "class_declaration" | "interface_declaration"),
        "cpp" => matches!(
            k,
            "class_specifier" | "struct_specifier" | "namespace_definition"
        ),
        "java" => matches!(
            k,
            "class_declaration"
                | "interface_declaration"
                | "enum_declaration"
                | "record_declaration"
        ),
        _ => false,
    }
}
fn is_call_kind(lang: &str, k: &str) -> bool {
    match lang {
        "python" => k == "call",
        "java" => k == "method_invocation",
        _ => k == "call_expression",
    }
}
fn is_dependency_kind(lang: &str, k: &str) -> bool {
    match lang {
        "rust" => k == "use_declaration",
        "python" => matches!(k, "import_statement" | "import_from_statement"),
        "javascript" | "typescript" => k == "import_statement",
        "go" => matches!(k, "import_declaration" | "import_spec"),
        "c" | "cpp" => k == "preproc_include",
        "java" => matches!(k, "import_declaration" | "package_declaration"),
        _ => false,
    }
}
fn symbol_name(node: Node<'_>, src: &[u8]) -> Option<String> {
    for f in ["name", "declarator", "type"] {
        if let Some(n) = node.child_by_field_name(f) {
            if let Some(x) = first_identifier(n, src, 0) {
                return Some(x);
            }
        }
    }
    first_identifier(node, src, 0)
}
fn first_identifier(node: Node<'_>, src: &[u8], depth: usize) -> Option<String> {
    if depth > 5 {
        return None;
    }
    let k = node.kind();
    if k.contains("identifier")
        || matches!(
            k,
            "field_identifier" | "type_identifier" | "namespace_identifier"
        )
    {
        return node.utf8_text(src).ok().map(str::to_string);
    }
    let mut c = node.walk();
    for ch in node.named_children(&mut c) {
        if let Some(x) = first_identifier(ch, src, depth + 1) {
            return Some(x);
        }
    }
    None
}
fn call_target(node: Node<'_>, src: &[u8]) -> Option<String> {
    for f in ["function", "name", "method"] {
        if let Some(n) = node.child_by_field_name(f) {
            return n.utf8_text(src).ok().map(str::to_string);
        }
    }
    node.named_child(0)
        .and_then(|n| n.utf8_text(src).ok().map(str::to_string))
}
fn normalize_target_name(s: &str) -> String {
    s.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|x| !x.is_empty())
        .last()
        .unwrap_or(s)
        .to_string()
}
fn bounded_text(node: Node<'_>, src: &[u8], max: usize) -> String {
    let t = node.utf8_text(src).unwrap_or("").trim().replace('\n', " ");
    t.chars().take(max).collect()
}
fn branch_complexity(node: Node<'_>) -> u32 {
    let mut n = 1u32;
    fn walk(x: Node<'_>, n: &mut u32) {
        if matches!(
            x.kind(),
            "if_expression"
                | "if_statement"
                | "for_expression"
                | "for_statement"
                | "while_expression"
                | "while_statement"
                | "match_expression"
                | "switch_statement"
                | "case_statement"
                | "conditional_expression"
                | "catch_clause"
        ) {
            *n = n.saturating_add(1)
        }
        let mut c = x.walk();
        for ch in x.named_children(&mut c) {
            walk(ch, n)
        }
    }
    walk(node, &mut n);
    n
}
fn walk_stats(node: Node<'_>, s: &mut (usize, usize)) {
    if node.is_error() || node.is_missing() {
        s.0 += 1
    }
    if node.is_named() {
        s.1 += 1
    }
    let mut c = node.walk();
    for ch in node.children(&mut c) {
        walk_stats(ch, s)
    }
}
fn range(n: Node<'_>) -> SourceRange {
    let a = n.start_position();
    let b = n.end_position();
    SourceRange {
        start_row: a.row,
        start_column: a.column,
        end_row: b.row,
        end_column: b.column,
    }
}
fn find_enclosing_symbol(file: &str, point: Point, symbols: &[RepoSymbol]) -> Option<String> {
    let mut candidates: Vec<&RepoSymbol> = symbols
        .iter()
        .filter(|s| {
            s.file_path == file
                && position_le(
                    s.range.start_row,
                    s.range.start_column,
                    point.row,
                    point.column,
                )
                && position_le(point.row, point.column, s.range.end_row, s.range.end_column)
        })
        .collect();
    candidates.sort_by_key(|s| {
        (
            s.range.end_row.saturating_sub(s.range.start_row),
            s.range.end_column.saturating_sub(s.range.start_column),
        )
    });
    candidates.first().map(|s| s.symbol_id.clone())
}
fn position_le(ar: usize, ac: usize, br: usize, bc: usize) -> bool {
    ar < br || (ar == br && ac <= bc)
}
fn resolve_calls(calls: &mut [RepoCall], symbols: &[RepoSymbol]) {
    let mut by = BTreeMap::<String, Vec<String>>::new();
    for s in symbols {
        by.entry(s.name.clone())
            .or_default()
            .push(s.symbol_id.clone())
    }
    for c in calls {
        c.resolved_symbol_ids = by.get(&c.target_name).cloned().unwrap_or_default();
        c.resolved_symbol_ids.sort();
    }
}
fn resolve_dependencies(deps: &mut [RepoDependency], files: &[RepoFile]) {
    let mut keys = BTreeMap::<String, String>::new();
    for f in files {
        let p = Path::new(&f.path);
        if let Some(stem) = p.file_stem().and_then(|x| x.to_str()) {
            keys.entry(stem.to_string())
                .or_insert_with(|| f.path.clone());
        }
        let noext = p.with_extension("").to_string_lossy().replace('\\', "/");
        keys.entry(noext.replace('/', "::"))
            .or_insert_with(|| f.path.clone());
        keys.entry(noext.replace('/', "."))
            .or_insert_with(|| f.path.clone());
    }
    for d in deps {
        let raw = d.raw.replace('"', " ").replace('\'', " ");
        let mut hits: Vec<_> = keys
            .iter()
            .filter(|(k, _)| k.len() > 1 && raw.contains(k.as_str()))
            .map(|(_, v)| v.clone())
            .filter(|v| v != &d.file_path)
            .collect();
        hits.sort();
        hits.dedup();
        d.resolved_file = hits.into_iter().next();
    }
}
fn compute_hotspots(
    inv: &RepoInventory,
    symbols: &[RepoSymbol],
    calls: &[RepoCall],
    deps: &[RepoDependency],
) -> Vec<RepoHotspot> {
    let paths: Vec<String> = inv.files.iter().map(|f| f.path.clone()).collect();
    let idx: BTreeMap<_, _> = paths
        .iter()
        .enumerate()
        .map(|(i, p)| (p.clone(), i))
        .collect();
    let symfile: BTreeMap<_, _> = symbols
        .iter()
        .map(|s| (s.symbol_id.clone(), s.file_path.clone()))
        .collect();
    let mut edges = BTreeSet::<(usize, usize)>::new();
    for c in calls {
        if let Some(&a) = idx.get(&c.file_path) {
            for id in &c.resolved_symbol_ids {
                if let Some(fp) = symfile.get(id) {
                    if let Some(&b) = idx.get(fp) {
                        if a != b {
                            edges.insert((a, b));
                        }
                    }
                }
            }
        }
    }
    for d in deps {
        if let (Some(&a), Some(fp)) = (idx.get(&d.file_path), d.resolved_file.as_ref()) {
            if let Some(&b) = idx.get(fp) {
                if a != b {
                    edges.insert((a, b));
                }
            }
        }
    }
    let n = paths.len();
    if n == 0 {
        return vec![];
    }
    let mut rank = vec![1.0 / n as f64; n];
    let damping = 0.85;
    for _ in 0..24 {
        let mut next = vec![(1.0 - damping) / n as f64; n];
        let mut out = vec![0usize; n];
        for (a, _) in &edges {
            out[*a] += 1
        }
        let dangling: f64 = (0..n).filter(|i| out[*i] == 0).map(|i| rank[i]).sum();
        for v in &mut next {
            *v += damping * dangling / n as f64
        }
        for (a, b) in &edges {
            next[*b] += damping * rank[*a] / out[*a] as f64
        }
        rank = next
    }
    let mut inbound = vec![0; n];
    let mut outbound = vec![0; n];
    for (a, b) in &edges {
        outbound[*a] += 1;
        inbound[*b] += 1
    }
    let mut res = Vec::new();
    for (i, p) in paths.iter().enumerate() {
        let syms: Vec<_> = symbols.iter().filter(|s| &s.file_path == p).collect();
        let complexity: u64 = syms.iter().map(|s| s.complexity as u64).sum();
        let base = inv
            .files
            .iter()
            .find(|f| &f.path == p)
            .map(|f| f.score)
            .unwrap_or(0);
        let ppm = (rank[i] * 1_000_000.0).round().clamp(0.0, 1_000_000.0) as u32;
        let score = base
            + complexity * 40
            + inbound[i] as u64 * 120
            + outbound[i] as u64 * 60
            + ppm as u64 / 100;
        res.push(RepoHotspot {
            file_path: p.clone(),
            pagerank_ppm: ppm,
            inbound_edges: inbound[i],
            outbound_edges: outbound[i],
            symbol_count: syms.len(),
            complexity,
            hotspot_score: score,
        })
    }
    res.sort_by(|a, b| {
        b.hotspot_score
            .cmp(&a.hotspot_score)
            .then_with(|| a.file_path.cmp(&b.file_path))
    });
    res
}
fn importance_score(path: &str, lines: usize, bytes: u64) -> u64 {
    let mut s = (lines as u64).min(5000) + (bytes / 1024).min(1000);
    for k in [
        "main.",
        "lib.",
        "mod.",
        "Cargo.toml",
        "package.json",
        "README",
        "schema",
        "migration",
        "config",
    ] {
        if path.contains(k) {
            s += 250
        }
    }
    s
}
fn detect(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "rs" => "rust",
        "py" => "python",
        "js" | "jsx" => "javascript",
        "ts" | "tsx" => "typescript",
        "go" => "go",
        "c" | "h" => "c",
        "cpp" | "hpp" | "cc" => "cpp",
        "java" => "java",
        "cs" => "csharp",
        "sql" => "sql",
        "toml" => "toml",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "md" => "markdown",
        "html" => "html",
        "css" => "css",
        "xml" => "xml",
        "" => "text",
        _ => "text",
    }
}
fn ignored(path: &Path, root: &Path) -> bool {
    path.strip_prefix(root).ok().is_some_and(|r| {
        r.components().any(|c| {
            matches!(
                c.as_os_str().to_str(),
                Some(".git" | "target" | "node_modules" | "var")
            )
        })
    })
}
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
fn stable_id(parts: &[&str]) -> String {
    let mut h = Sha256::new();
    for p in parts {
        h.update((p.len() as u64).to_be_bytes());
        h.update(p.as_bytes())
    }
    format!("{:x}", h.finalize())
}
fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn rust_tree_sitter_extracts_symbols_and_calls() {
        let dir = std::env::temp_dir().join(format!("phx-repo-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        let mut f = fs::File::create(dir.join("src/lib.rs")).unwrap();
        writeln!(f, "pub fn alpha() {{ beta(); }}\nfn beta() {{}}").unwrap();
        let r = analyze(&dir);
        assert!(r.parses.iter().any(|p| p.status == ParseStatus::Parsed));
        assert!(r.symbols.iter().any(|s| s.name == "alpha"));
        assert!(
            r.calls
                .iter()
                .any(|c| c.target_name == "beta" && !c.resolved_symbol_ids.is_empty())
        );
        let _ = fs::remove_dir_all(dir);
    }
    #[test]
    fn python_tree_sitter_extracts_class_and_method() {
        let dir = std::env::temp_dir().join(format!("phx-repo-py-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("a.py"),
            "class A:\n    def run(self):\n        helper()\ndef helper():\n    pass\n",
        )
        .unwrap();
        let r = analyze(&dir);
        assert!(r.symbols.iter().any(|s| s.name == "A"));
        assert!(r.symbols.iter().any(|s| s.name == "run"));
        assert!(r.calls.iter().any(|c| c.target_name == "helper"));
        let _ = fs::remove_dir_all(dir);
    }
}
