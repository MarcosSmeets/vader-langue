//! `vader lsp` — Language Server over stdio.
//!
//! Reuses the SAME lexer/parser/checker as the compiler. It publishes diagnostics
//! (parse and type errors, with line:column) and answers `hover`, `definition` and
//! `documentSymbol`. Document sync is "full" (the editor sends the whole text on
//! each change); the latest text per document is kept so position-based requests
//! can be answered.

use std::collections::HashMap;
use std::io::{self, BufRead, Write};

use crate::ast;
use crate::json::Json;
use crate::token::TokenKind;
use crate::{check, lexer, parser};

/// Holds the latest text of each open document, keyed by URI.
#[derive(Default)]
pub struct Server {
    docs: HashMap<String, String>,
}

/// Runs the server's IO loop until EOF or `exit`.
pub fn run() {
    let stdin = io::stdin();
    let mut r = stdin.lock();
    let stdout = io::stdout();
    let mut w = stdout.lock();
    let mut server = Server::default();
    while let Some(msg) = read_message(&mut r) {
        for payload in server.handle(&msg) {
            let _ = write!(w, "Content-Length: {}\r\n\r\n{}", payload.len(), payload);
            let _ = w.flush();
        }
        if method_of(&msg) == "exit" {
            break;
        }
    }
}

/// Reads an LSP message (`Content-Length` headers + body). `None` at EOF.
fn read_message(r: &mut impl BufRead) -> Option<String> {
    let mut len = 0usize;
    loop {
        let mut line = String::new();
        let n = r.read_line(&mut line).ok()?;
        if n == 0 {
            return None; // EOF
        }
        let t = line.trim_end();
        if t.is_empty() {
            break; // end of headers
        }
        if let Some(v) = t.strip_prefix("Content-Length:") {
            len = v.trim().parse().ok()?;
        }
    }
    let mut body = vec![0u8; len];
    io::Read::read_exact(r, &mut body).ok()?;
    String::from_utf8(body).ok()
}

fn method_of(msg: &str) -> String {
    crate::json::parse(msg)
        .and_then(|j| j.get("method").and_then(|m| m.as_str()).map(String::from))
        .unwrap_or_default()
}

impl Server {
    /// Processes a raw JSON-RPC message and returns the payloads to send back.
    pub fn handle(&mut self, msg: &str) -> Vec<String> {
        let json = match crate::json::parse(msg) {
            Some(j) => j,
            None => return vec![],
        };
        let method = json.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let id = || json.get("id").cloned().unwrap_or(Json::Null);
        match method {
            "initialize" => vec![response(id(), init_result())],
            "textDocument/didOpen" => open_params(&json)
                .map(|(uri, text)| {
                    self.docs.insert(uri.clone(), text.clone());
                    vec![publish_payload(&uri, &text)]
                })
                .unwrap_or_default(),
            "textDocument/didChange" => change_params(&json)
                .map(|(uri, text)| {
                    self.docs.insert(uri.clone(), text.clone());
                    vec![publish_payload(&uri, &text)]
                })
                .unwrap_or_default(),
            "textDocument/hover" => vec![response(id(), self.hover(&json))],
            "textDocument/definition" => vec![response(id(), self.definition(&json))],
            "textDocument/documentSymbol" => vec![response(id(), self.document_symbols(&json))],
            "shutdown" => vec![response(id(), Json::Null)],
            _ => vec![],
        }
    }

    /// `textDocument/hover` — for a user-declared symbol under the cursor, show its
    /// reconstructed signature. (Stdlib calls are covered by the client extension.)
    fn hover(&self, j: &Json) -> Json {
        let (text, line, ch) = match self.req_doc_pos(j) {
            Some(t) => t,
            None => return Json::Null,
        };
        let name = match ident_at(text, line, ch) {
            Some(n) => n,
            None => return Json::Null,
        };
        match signature_of(text, &name) {
            Some(sig) => Json::Obj(vec![(
                "contents".into(),
                Json::Obj(vec![
                    ("kind".into(), Json::Str("markdown".into())),
                    ("value".into(), Json::Str(format!("```vader\n{}\n```", sig))),
                ]),
            )]),
            None => Json::Null,
        }
    }

    /// `textDocument/definition` — jump to a top-level declaration with the same name.
    fn definition(&self, j: &Json) -> Json {
        let (text, line, ch) = match self.req_doc_pos(j) {
            Some(t) => t,
            None => return Json::Null,
        };
        let uri = req_uri(j).unwrap_or_default();
        let name = match ident_at(text, line, ch) {
            Some(n) => n,
            None => return Json::Null,
        };
        let locs: Vec<Json> = symbols(text)
            .into_iter()
            .filter(|s| s.name == name)
            .map(|s| {
                Json::Obj(vec![
                    ("uri".into(), Json::Str(uri.clone())),
                    ("range".into(), lsp_range(s.line, s.col, s.name.len())),
                ])
            })
            .collect();
        if locs.is_empty() {
            Json::Null
        } else {
            Json::Arr(locs)
        }
    }

    /// `textDocument/documentSymbol` — outline of top-level declarations.
    fn document_symbols(&self, j: &Json) -> Json {
        let uri = match req_uri(j) {
            Some(u) => u,
            None => return Json::Null,
        };
        let text = match self.docs.get(&uri) {
            Some(t) => t,
            None => return Json::Null,
        };
        let arr: Vec<Json> = symbols(text)
            .into_iter()
            .map(|s| {
                let range = lsp_range(s.line, s.col, s.name.len());
                Json::Obj(vec![
                    ("name".into(), Json::Str(s.name)),
                    ("kind".into(), Json::Num(s.kind as f64)),
                    ("range".into(), range.clone()),
                    ("selectionRange".into(), range),
                ])
            })
            .collect();
        Json::Arr(arr)
    }

    /// Resolves a request's (document text, 0-based line, 0-based character).
    fn req_doc_pos(&self, j: &Json) -> Option<(&str, usize, usize)> {
        let uri = req_uri(j)?;
        let text = self.docs.get(&uri)?;
        let (line, ch) = req_pos(j)?;
        Some((text, line, ch))
    }
}

fn init_result() -> Json {
    Json::Obj(vec![
        (
            "capabilities".into(),
            Json::Obj(vec![
                // 1 = "full" sync: the editor resends the whole text on each change
                ("textDocumentSync".into(), Json::Num(1.0)),
                ("hoverProvider".into(), Json::Bool(true)),
                ("definitionProvider".into(), Json::Bool(true)),
                ("documentSymbolProvider".into(), Json::Bool(true)),
            ]),
        ),
        (
            "serverInfo".into(),
            Json::Obj(vec![("name".into(), Json::Str("vader-lsp".into()))]),
        ),
    ])
}

fn response(id: Json, result: Json) -> String {
    Json::Obj(vec![
        ("jsonrpc".into(), Json::Str("2.0".into())),
        ("id".into(), id),
        ("result".into(), result),
    ])
    .to_string()
}

fn open_params(j: &Json) -> Option<(String, String)> {
    let td = j.get("params")?.get("textDocument")?;
    let uri = td.get("uri")?.as_str()?.to_string();
    let text = td.get("text")?.as_str()?.to_string();
    Some((uri, text))
}

fn change_params(j: &Json) -> Option<(String, String)> {
    let params = j.get("params")?;
    let uri = params.get("textDocument")?.get("uri")?.as_str()?.to_string();
    let last = params.get("contentChanges")?.as_array()?.last()?;
    let text = last.get("text")?.as_str()?.to_string();
    Some((uri, text))
}

fn req_uri(j: &Json) -> Option<String> {
    Some(j.get("params")?.get("textDocument")?.get("uri")?.as_str()?.to_string())
}

fn req_pos(j: &Json) -> Option<(usize, usize)> {
    let p = j.get("params")?.get("position")?;
    let num = |k: &str| match p.get(k) {
        Some(Json::Num(n)) => Some(*n as usize),
        _ => None,
    };
    Some((num("line")?, num("character")?))
}

/// Builds an LSP range (0-based) for a name starting at 1-based (line, col).
fn lsp_range(line1: usize, col1: usize, len: usize) -> Json {
    let l = line1.saturating_sub(1) as f64;
    let c0 = col1.saturating_sub(1) as f64;
    let pos = |line: f64, ch: f64| {
        Json::Obj(vec![
            ("line".into(), Json::Num(line)),
            ("character".into(), Json::Num(ch)),
        ])
    };
    Json::Obj(vec![
        ("start".into(), pos(l, c0)),
        ("end".into(), pos(l, c0 + len as f64)),
    ])
}

/// A top-level declaration with the position of its name token (1-based).
struct Sym {
    name: String,
    kind: u8, // LSP SymbolKind
    line: usize,
    col: usize,
}

// LSP SymbolKind values.
const SK_METHOD: u8 = 6;
const SK_ENUM: u8 = 10;
const SK_INTERFACE: u8 = 11;
const SK_FUNCTION: u8 = 12;
const SK_STRUCT: u8 = 23;

/// Extracts top-level declarations by scanning the token stream (the AST carries no
/// positions, but tokens do). Works even when the file doesn't fully parse.
fn symbols(text: &str) -> Vec<Sym> {
    let tokens = match lexer::tokenize(text) {
        Ok(t) => t,
        Err(_) => return vec![],
    };
    let mut out = Vec::new();
    let push_named = |out: &mut Vec<Sym>, t: Option<&crate::token::Token>, kind: u8| {
        if let Some(tok) = t {
            if let TokenKind::Ident(name) = &tok.kind {
                out.push(Sym { name: name.clone(), kind, line: tok.line, col: tok.col });
            }
        }
    };
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i].kind {
            TokenKind::Fn => {
                // Optional method receiver: `fn (u User) name(...)` — skip the `( ... )`.
                let mut j = i + 1;
                let is_method = matches!(tokens.get(j).map(|t| &t.kind), Some(TokenKind::LParen));
                if is_method {
                    let mut depth = 0;
                    while j < tokens.len() {
                        match tokens[j].kind {
                            TokenKind::LParen => depth += 1,
                            TokenKind::RParen => {
                                depth -= 1;
                                if depth == 0 {
                                    j += 1;
                                    break;
                                }
                            }
                            _ => {}
                        }
                        j += 1;
                    }
                }
                // A name here means a declaration; otherwise it's a `fn(...)` type — skip.
                push_named(&mut out, tokens.get(j), if is_method { SK_METHOD } else { SK_FUNCTION });
            }
            TokenKind::Struct => push_named(&mut out, tokens.get(i + 1), SK_STRUCT),
            TokenKind::Interface => push_named(&mut out, tokens.get(i + 1), SK_INTERFACE),
            TokenKind::Enum => push_named(&mut out, tokens.get(i + 1), SK_ENUM),
            TokenKind::Test => {
                if let Some(tok) = tokens.get(i + 1) {
                    if let TokenKind::Str(name) = &tok.kind {
                        out.push(Sym { name: name.clone(), kind: SK_FUNCTION, line: tok.line, col: tok.col });
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// Returns the identifier name at the given 0-based (line, character), if any.
fn ident_at(text: &str, line0: usize, char0: usize) -> Option<String> {
    let tokens = lexer::tokenize(text).ok()?;
    for t in &tokens {
        if let TokenKind::Ident(name) = &t.kind {
            let tl = t.line.saturating_sub(1);
            let tc = t.col.saturating_sub(1);
            if tl == line0 && char0 >= tc && char0 <= tc + name.len() {
                return Some(name.clone());
            }
        }
    }
    None
}

/// Reconstructs a readable signature for a top-level declaration named `name`.
fn signature_of(text: &str, name: &str) -> Option<String> {
    let tokens = lexer::tokenize(text).ok()?;
    let prog = parser::parse(tokens).ok()?;
    for item in &prog.items {
        match item {
            ast::Item::Function(f) if f.name == name => return Some(fn_sig(f)),
            ast::Item::Struct(s) if s.name == name => return Some(struct_sig(s)),
            ast::Item::Interface(it) if it.name == name => return Some(iface_sig(it)),
            ast::Item::Enum(e) if e.name == name => return Some(enum_sig(e)),
            _ => {}
        }
    }
    None
}

fn type_str(t: &ast::Type) -> String {
    match t {
        ast::Type::Named(n) => n.clone(),
        ast::Type::Generic(n, args) => {
            format!("{}[{}]", n, args.iter().map(type_str).collect::<Vec<_>>().join(", "))
        }
        ast::Type::Slice(inner) => format!("[]{}", type_str(inner)),
    }
}

fn params_str(params: &[ast::Param]) -> String {
    params.iter().map(|p| format!("{} {}", p.name, type_str(&p.ty))).collect::<Vec<_>>().join(", ")
}

fn returns_str(returns: &[ast::Type]) -> String {
    if returns.is_empty() {
        return String::new();
    }
    let rs: Vec<String> = returns.iter().map(type_str).collect();
    if rs.len() == 1 {
        format!(": {}", rs[0])
    } else {
        format!(": ({})", rs.join(", "))
    }
}

fn fn_sig(f: &ast::Function) -> String {
    let mut s = String::new();
    if f.visibility == ast::Visibility::Public {
        s.push_str("public ");
    }
    s.push_str("fn ");
    if let Some(r) = &f.receiver {
        s.push_str(&format!("({} {}) ", r.name, type_str(&r.ty)));
    }
    s.push_str(&f.name);
    if !f.type_params.is_empty() {
        let tps: Vec<String> = f.type_params.iter().map(|t| t.name.clone()).collect();
        s.push_str(&format!("[{}]", tps.join(", ")));
    }
    s.push_str(&format!("({})", params_str(&f.params)));
    s.push_str(&returns_str(&f.returns));
    s
}

fn struct_sig(s: &ast::StructDef) -> String {
    let mut out = String::new();
    if s.visibility == ast::Visibility::Public {
        out.push_str("public ");
    }
    out.push_str(&format!("struct {} {{", s.name));
    for f in &s.fields {
        out.push_str(&format!("\n    {} {}", f.name, type_str(&f.ty)));
    }
    if !s.fields.is_empty() {
        out.push('\n');
    }
    out.push('}');
    out
}

fn enum_sig(e: &ast::EnumDef) -> String {
    let mut out = String::new();
    if e.visibility == ast::Visibility::Public {
        out.push_str("public ");
    }
    out.push_str(&format!("enum {} {{", e.name));
    for v in &e.variants {
        if v.fields.is_empty() {
            out.push_str(&format!("\n    {}", v.name));
        } else {
            out.push_str(&format!("\n    {}({})", v.name, params_str(&v.fields)));
        }
    }
    if !e.variants.is_empty() {
        out.push('\n');
    }
    out.push('}');
    out
}

fn iface_sig(it: &ast::InterfaceDef) -> String {
    let mut out = String::new();
    if it.visibility == ast::Visibility::Public {
        out.push_str("public ");
    }
    out.push_str(&format!("interface {} {{", it.name));
    for m in &it.methods {
        out.push_str(&format!("\n    fn {}({}){}", m.name, params_str(&m.params), returns_str(&m.returns)));
    }
    if !it.methods.is_empty() {
        out.push('\n');
    }
    out.push('}');
    out
}

/// Builds the `textDocument/publishDiagnostics` notification for a document.
fn publish_payload(uri: &str, text: &str) -> String {
    let arr: Vec<Json> = diagnostics(text)
        .into_iter()
        .map(|(line, col, msg)| {
            let l = line.saturating_sub(1) as f64; // LSP is 0-based
            let c = col.saturating_sub(1) as f64;
            Json::Obj(vec![
                (
                    "range".into(),
                    Json::Obj(vec![
                        (
                            "start".into(),
                            Json::Obj(vec![
                                ("line".into(), Json::Num(l)),
                                ("character".into(), Json::Num(c)),
                            ]),
                        ),
                        (
                            "end".into(),
                            Json::Obj(vec![
                                ("line".into(), Json::Num(l)),
                                ("character".into(), Json::Num(c + 1.0)),
                            ]),
                        ),
                    ]),
                ),
                ("severity".into(), Json::Num(1.0)), // 1 = Error
                ("source".into(), Json::Str("vader".into())),
                ("message".into(), Json::Str(msg)),
            ])
        })
        .collect();
    Json::Obj(vec![
        ("jsonrpc".into(), Json::Str("2.0".into())),
        (
            "method".into(),
            Json::Str("textDocument/publishDiagnostics".into()),
        ),
        (
            "params".into(),
            Json::Obj(vec![
                ("uri".into(), Json::Str(uri.to_string())),
                ("diagnostics".into(), Json::Arr(arr)),
            ]),
        ),
    ])
    .to_string()
}

/// Runs the pipeline and returns (line, column, message) — 1-based, like the compiler.
fn diagnostics(src: &str) -> Vec<(usize, usize, String)> {
    let tokens = match lexer::tokenize(src) {
        Ok(t) => t,
        Err(e) => return vec![(e.line, e.col, e.message)],
    };
    let prog = match parser::parse(tokens) {
        Ok(p) => p,
        Err(e) => return vec![(e.line, e.col, e.message)],
    };
    match check::check(&prog) {
        Ok(()) => vec![],
        Err(errs) => errs
            .into_iter()
            .map(|e| (e.line, e.col, e.message))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handle(msg: &str) -> Vec<String> {
        Server::default().handle(msg)
    }

    #[test]
    fn initialize_advertises_capabilities() {
        let out = handle(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("textDocumentSync"));
        assert!(out[0].contains("hoverProvider"));
        assert!(out[0].contains("definitionProvider"));
        assert!(out[0].contains("documentSymbolProvider"));
        assert!(out[0].contains("\"id\":1"));
    }

    #[test]
    fn didopen_publishes_diagnostics_for_bad_code() {
        let msg = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///x.vd","text":"fn main() { nope() }"}}}"#;
        let out = handle(msg);
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("publishDiagnostics"));
        assert!(out[0].contains("file:///x.vd"));
        assert!(out[0].contains("\"severity\":1"));
    }

    #[test]
    fn didopen_clean_code_has_empty_diagnostics() {
        let msg = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///ok.vd","text":"fn main() { print(1) }"}}}"#;
        let out = handle(msg);
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("\"diagnostics\":[]"));
    }

    #[test]
    fn didchange_uses_last_content() {
        let msg = r#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///x.vd"},"contentChanges":[{"text":"fn main() { print(1) }"}]}}"#;
        let out = handle(msg);
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("\"diagnostics\":[]"));
    }

    #[test]
    fn document_symbols_lists_declarations() {
        let mut s = Server::default();
        let src = "struct User { id int }\\npublic fn greet(name string): string { return name }";
        let open = format!(
            r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///a.vd","text":"{}"}}}}}}"#,
            src
        );
        s.handle(&open);
        let req = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/documentSymbol","params":{"textDocument":{"uri":"file:///a.vd"}}}"#;
        let out = s.handle(req);
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("\"name\":\"User\""));
        assert!(out[0].contains("\"name\":\"greet\""));
        assert!(out[0].contains(&format!("\"kind\":{}", SK_STRUCT)));
    }

    #[test]
    fn definition_points_at_the_function() {
        let mut s = Server::default();
        // line 0: `fn helper() { }`  line 1: `fn main() { helper() }`
        let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///b.vd","text":"fn helper() { }\nfn main() { helper() }"}}}"#;
        s.handle(open);
        // cursor on the `helper` call in line 1 (character 12)
        let req = r#"{"jsonrpc":"2.0","id":3,"method":"textDocument/definition","params":{"textDocument":{"uri":"file:///b.vd"},"position":{"line":1,"character":13}}}"#;
        let out = s.handle(req);
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("file:///b.vd"));
        assert!(out[0].contains("\"line\":0")); // defined on the first line
    }

    #[test]
    fn hover_shows_a_signature() {
        let mut s = Server::default();
        let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///c.vd","text":"public fn add(a int, b int): int { return a }\nfn main() { add(1, 2) }"}}}"#;
        s.handle(open);
        let req = r#"{"jsonrpc":"2.0","id":4,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///c.vd"},"position":{"line":1,"character":12}}}"#;
        let out = s.handle(req);
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("public fn add"));
        assert!(out[0].contains("markdown"));
    }
}
