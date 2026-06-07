# Changelog

## 0.6.0

- **Run / Build / Test from the editor** — CodeLens **▶ Run · Build · LLVM** above
  `fn main()` and **▶ Run tests** above `test "…"` blocks; *Vader: Run/Build/Test File*
  and *Test Project* commands in the Command Palette, right-click menu, and the editor
  title ▶ menu.
- **Formatting** — document formatter backed by `vader fmt` (`Shift+Alt+F` and
  format-on-save); files with a syntax error are left untouched.
- **Hover & signature help** — stdlib calls (`db.`/`http.`/`json.`/`mem.` and
  `newRouter`/`serve`) show their signature on hover and parameter hints while typing,
  plus an *Add import "std/…"* quick-fix.
- **Test Explorer** — `test "…"` blocks appear in the VS Code Testing panel and run
  via `vader test`, with per-test pass/fail.
- Marketplace: added Snippets/Formatters/Linters categories and a gallery banner.

## 0.5.0

- **English UI** — every user-facing string (description, settings, command
  titles, the right-click *Vader: Generate* menu, prompts and error messages) is
  now in English.
- **Code snippets** — 39 snippets: declarations (`pfn`, `fn`, `struct`,
  `interface`, `enum`, `match`, `iferr`, `for…`, `test`, `usecase`, `handler`),
  an HTTP router set (`router`, `route`, `handlerfn`, `newrouter`, `serve`), and
  stdlib starters (`httpserver`, `dbquery`, `jsonbuild`).
- **Stdlib completion with auto-import** — typing `db.`, `http.`, `json.` or
  `mem.` completes the built-in functions and inserts the matching
  `import "std/…"` if it's missing. `newRouter` and `serve` are offered as
  globals that auto-import `std/http`.
- **File icon** — `.vd` files now show the bare *Visor* mark on a transparent
  background (no document silhouette), legible on light and dark themes.

## 0.4.0

- **Brand icon** — *Visor* marketplace icon and `.vd` file icons.

## 0.3.0

- Syntax highlighting, language server client (`vader lsp`), and right-click code
  generation (struct / use case / handler / function).
