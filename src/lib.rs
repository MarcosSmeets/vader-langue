//! The Vader compiler library.
//!
//! Phase 1: lexer + parser + type checker + backend (transpile-to-Go).
//! See `SPEC.md` and `docs/` for the design.

pub mod ast;
pub mod check;
pub mod codegen;
pub mod formatter;
pub mod gen;
pub mod json;
pub mod lexer;
pub mod lint;
pub mod llvm;
pub mod lsp;
pub mod migrate;
pub mod module;
pub mod parser;
pub mod pkg;
pub mod scaffold;
pub mod templates;
pub mod token;
