# Vader — VS Code extension

What it gives you:
1. **Syntax highlighting** (`.vd`) — works everywhere, no dependencies.
2. **Language Server** — real-time parse and type errors, reusing the
   compiler (`vader lsp`). Requires the `vader` binary and an `npm install`.
3. **Snippets** — `pfn`, `fn`, `struct`, `interface`, `enum`, `match`, `iferr`,
   `for…`, `test`, `usecase`, `handler`; HTTP router (`router`, `route`,
   `handlerfn`, `newrouter`, `serve`); and stdlib starters (`httpserver`,
   `dbquery`, `jsonbuild`). Type a prefix and press Tab.
4. **Stdlib completion with auto-import** — type `db.`, `http.`, `json.` or
   `mem.` to complete the built-in functions; accepting one inserts the matching
   `import "std/…"` automatically if it's missing.
5. **Run / Build / Test** — CodeLens **▶ Run · Build · LLVM** above `fn main()` and
   **▶ Run tests** above `test "…"` blocks, plus *Vader: Run/Build/Test File* commands
   (Command Palette, right-click, and the editor title ▶ menu). Runs in a terminal.
6. **Formatting** — `Shift+Alt+F` (or format-on-save) runs `vader fmt`; a file with a
   syntax error is left untouched.
7. **Hover & signature help** — hover a stdlib call (e.g. `db.exec`) for its signature,
   and get parameter hints while typing. Plus an *Add import "std/…"* quick-fix. These
   work without the `vader` binary.
8. **Test Explorer** — `test "…"` blocks show up in the Testing panel and run via
   `vader test`, with per-test pass/fail.

> Items 1–2 and 5–6 use the `vader` binary (a Linux/WSL build) — they work where
> `vader` is runnable (Remote-WSL or a native build); set `vader.serverPath` if it's
> not on your `PATH`. Snippets, completion, hover, and quick-fixes need no binary.

## 1) Syntax highlighting (no setup)

Highlights comments, keywords, types (`int float bool string error chan map`),
strings, numbers, function names, and operators (`<-`, `..`, `->`…). Plus `Ctrl+/`
and auto-closing of `{ [ ( "`.

It works as soon as the extension loads — see "Running" below.

## 2) Language Server (real-time errors)

The server is the **compiler itself**: `vader lsp` speaks the Language Server Protocol
over stdio and publishes diagnostics with line:column (the same ones as `vader check`).
The client here only launches the process — no editor-side reimplementation of analysis.

Install the client dependencies (once):
```bash
cd editors/vscode
npm install
```

### ⚠️ WSL: `vader` is a Linux binary

The Vader toolchain is built on **WSL** (Linux ELF), so VS Code on **Windows**
can't run `vader` directly. Options, from best to simplest:

- **Recommended — VS Code + Remote-WSL:** open the project inside WSL
  (`code .` from Ubuntu, or "Reopen in WSL"). The extension then runs in the
  Linux context and can see `vader`. Set the binary path if it isn't on the PATH:
  ```jsonc
  // settings.json
  "vader.serverPath": "/mnt/c/Users/marco/Documents/workspace/side_projects/vader/target/debug/vader"
  ```
- **Highlighting only:** turn the server off and use highlighting alone:
  ```jsonc
  "vader.enableLanguageServer": false
  ```
- **Native Windows build:** if you ever build a `vader.exe`, point
  `vader.serverPath` at it.

## Running (dev mode)

1. Open the `editors/vscode` folder in VS Code (in the right context — see above).
2. `npm install` (only if you want the language server).
3. Press **`F5`** → opens a window with the extension loaded.
4. Open a `.vd` file (e.g. `examples/shapes.vd`). Highlighting appears immediately; if
   the server is on, errors are underlined as you type.

Or install it locally by copying the folder to `~/.vscode/extensions/vader-0.5.0` and
reopening VS Code.

## Settings

| Setting | Default | What it does |
|---|---|---|
| `vader.serverPath` | `vader` | Path to the executable used as `vader lsp`. |
| `vader.enableLanguageServer` | `true` | Enables/disables real-time diagnostics. |
