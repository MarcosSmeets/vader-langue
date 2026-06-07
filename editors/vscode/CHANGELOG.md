# Changelog

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
