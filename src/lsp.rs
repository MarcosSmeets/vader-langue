//! `vader lsp` — Language Server por stdio.
//!
//! Reusa o MESMO lexer/parser/checker do compilador e publica diagnósticos
//! (erros de parse e de tipo, com linha:coluna) no editor. Sem reimplementar
//! análise. Sync de documento é "full" (o editor manda o texto inteiro a cada
//! mudança), então cada `didOpen`/`didChange` recalcula tudo.

use std::io::{self, BufRead, Write};

use crate::json::Json;
use crate::{check, lexer, parser};

/// Roda o loop de IO do servidor até EOF ou `exit`.
pub fn run() {
    let stdin = io::stdin();
    let mut r = stdin.lock();
    let stdout = io::stdout();
    let mut w = stdout.lock();
    while let Some(msg) = read_message(&mut r) {
        for payload in handle(&msg) {
            let _ = write!(w, "Content-Length: {}\r\n\r\n{}", payload.len(), payload);
            let _ = w.flush();
        }
        if method_of(&msg) == "exit" {
            break;
        }
    }
}

/// Lê uma mensagem LSP (cabeçalhos `Content-Length` + corpo). `None` no EOF.
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
            break; // fim dos cabeçalhos
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

/// Processa uma mensagem JSON-RPC crua e devolve os payloads a enviar de volta.
/// Stateless (o texto vem na própria mensagem) — fácil de testar.
pub fn handle(msg: &str) -> Vec<String> {
    let json = match crate::json::parse(msg) {
        Some(j) => j,
        None => return vec![],
    };
    let method = json.get("method").and_then(|m| m.as_str()).unwrap_or("");
    match method {
        "initialize" => {
            let id = json.get("id").cloned().unwrap_or(Json::Null);
            vec![response(id, init_result())]
        }
        "textDocument/didOpen" => open_params(&json)
            .map(|(uri, text)| vec![publish_payload(&uri, &text)])
            .unwrap_or_default(),
        "textDocument/didChange" => change_params(&json)
            .map(|(uri, text)| vec![publish_payload(&uri, &text)])
            .unwrap_or_default(),
        "shutdown" => {
            let id = json.get("id").cloned().unwrap_or(Json::Null);
            vec![response(id, Json::Null)]
        }
        _ => vec![],
    }
}

fn init_result() -> Json {
    Json::Obj(vec![
        (
            "capabilities".into(),
            Json::Obj(vec![
                // 1 = sync "full": o editor reenvia o texto inteiro a cada mudança
                ("textDocumentSync".into(), Json::Num(1.0)),
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

/// Monta a notificação `textDocument/publishDiagnostics` para um documento.
fn publish_payload(uri: &str, text: &str) -> String {
    let arr: Vec<Json> = diagnostics(text)
        .into_iter()
        .map(|(line, col, msg)| {
            let l = line.saturating_sub(1) as f64; // LSP é 0-based
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

/// Roda o pipeline e devolve (linha, coluna, mensagem) — 1-based, como o compilador.
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

    #[test]
    fn initialize_advertises_capabilities() {
        let out = handle(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("textDocumentSync"));
        assert!(out[0].contains("\"id\":1"));
    }

    #[test]
    fn didopen_publishes_diagnostics_for_bad_code() {
        // erro de tipo: chama função inexistente
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
}
