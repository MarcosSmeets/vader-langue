// Vader extension client:
//  (1) connects the editor to the `vader lsp` language server;
//  (2) code-generation commands (struct/usecase/handler/fn) in the right-click menu.
// Plain JavaScript (no TS build). Needs `npm install` once (only for the LSP).

const {
  workspace, window, commands, languages,
  CompletionItem, CompletionItemKind, SnippetString, TextEdit, Position,
} = require("vscode");
const { LanguageClient, TransportKind } = require("vscode-languageclient/node");

let client;

function activate(context) {
  registerGenCommands(context);
  registerStdCompletion(context);
  startLanguageServer();
}

// --- (1) Language Server -----------------------------------------------------

function startLanguageServer() {
  const config = workspace.getConfiguration("vader");
  if (!config.get("enableLanguageServer", true)) {
    return; // highlighting + generation only
  }
  const command = config.get("serverPath", "vader");
  const serverOptions = {
    run: { command, args: ["lsp"], transport: TransportKind.stdio },
    debug: { command, args: ["lsp"], transport: TransportKind.stdio },
  };
  const clientOptions = {
    documentSelector: [{ scheme: "file", language: "vader" }],
    synchronize: { fileEvents: workspace.createFileSystemWatcher("**/*.vd") },
  };
  client = new LanguageClient(
    "vader",
    "Vader Language Server",
    serverOptions,
    clientOptions
  );
  client.start().catch((err) => {
    window.showErrorMessage(
      `Vader: couldn't start the language server (${command} lsp). ` +
        `Adjust "vader.serverPath" in your settings. Details: ${err.message}`
    );
  });
}

// --- (2) Code generation (scaffolding) ---------------------------------------

function registerGenCommands(context) {
  const gens = [
    ["vader.gen.struct", "struct", "struct"],
    ["vader.gen.usecase", "usecase", "use case"],
    ["vader.gen.handler", "handler", "handler"],
    ["vader.gen.fn", "fn", "function"],
  ];
  for (const [id, thing, label] of gens) {
    const disposable = commands.registerCommand(id, () => runGen(thing, label));
    context.subscriptions.push(disposable);
  }
}

async function runGen(thing, label) {
  const name = await window.showInputBox({
    prompt: `Name of the ${label} to generate (vader gen ${thing} <Name>)`,
    placeHolder: "e.g. User",
    validateInput: (v) =>
      /^[A-Za-z_][A-Za-z0-9_]*$/.test(v.trim())
        ? null
        : "Use a valid identifier (letters, digits, _; must not start with a digit).",
  });
  if (!name) return;

  const bin = workspace.getConfiguration("vader").get("serverPath", "vader");
  // run in the integrated terminal to use the user's environment (PATH/WSL) and show output.
  const term =
    window.terminals.find((t) => t.name === "Vader") ||
    window.createTerminal("Vader");
  term.show();
  term.sendText(`${bin} gen ${thing} ${name.trim()}`);
}

// --- (3) Stdlib completion with automatic imports --------------------------
// Vader's stdlib is a fixed set of built-in modules. Typing `db.`, `http.`,
// `json.` or `mem.` offers their functions; accepting one also inserts the
// matching `import "std/..."` at the top of the file if it isn't there yet.
// (The `vader lsp` server only does diagnostics, so this lives client-side.)

const STD_MODULES = {
  db: {
    path: "std/db",
    items: [
      { name: "open", sig: "open(path string): DB", args: ["path"] },
      { name: "exec", sig: "exec(conn DB, sql string): error", args: ["conn", "sql"] },
      { name: "must", sig: "must(conn DB, sql string)", args: ["conn", "sql"] },
      { name: "query", sig: "query(conn DB, sql string): Rows", args: ["conn", "sql"] },
      { name: "next", sig: "next(rows Rows): bool", args: ["rows"] },
      { name: "col_int", sig: "col_int(rows Rows, i int): int", args: ["rows", "i"] },
      { name: "col_text", sig: "col_text(rows Rows, i int): string", args: ["rows", "i"] },
      { name: "col_float", sig: "col_float(rows Rows, i int): float", args: ["rows", "i"] },
      { name: "close", sig: "close(conn DB)", args: ["conn"] },
    ],
  },
  http: {
    path: "std/http",
    items: [
      { name: "listen", sig: "listen(port int): Server", args: ["port"] },
      { name: "accept", sig: "accept(s Server): bool", args: ["s"] },
      { name: "method", sig: "method(s Server): string", args: ["s"] },
      { name: "path", sig: "path(s Server): string", args: ["s"] },
      { name: "body", sig: "body(s Server): string", args: ["s"] },
      { name: "header", sig: "header(s Server, name string): string", args: ["s", "name"] },
      { name: "respond", sig: "respond(s Server, code int, contentType string, body string)", args: ["s", "code", "contentType", "body"] },
      { name: "get", sig: "get(url string): string", args: ["url"] },
      { name: "post", sig: "post(url string, contentType string, body string): string", args: ["url", "contentType", "body"] },
    ],
  },
  json: {
    path: "std/json",
    items: [
      { name: "parse", sig: "parse(text string): Json", args: ["text"] },
      { name: "field", sig: "field(obj Json, key string): Json", args: ["obj", "key"] },
      { name: "elem", sig: "elem(arr Json, i int): Json", args: ["arr", "i"] },
      { name: "as_str", sig: "as_str(v Json): string", args: ["v"] },
      { name: "as_int", sig: "as_int(v Json): int", args: ["v"] },
      { name: "as_float", sig: "as_float(v Json): float", args: ["v"] },
      { name: "as_bool", sig: "as_bool(v Json): bool", args: ["v"] },
      { name: "count", sig: "count(v Json): int", args: ["v"] },
      { name: "object", sig: "object(): Json", args: [] },
      { name: "array", sig: "array(): Json", args: [] },
      { name: "set", sig: "set(obj Json, key string, val Json): Json", args: ["obj", "key", "val"] },
      { name: "set_str", sig: "set_str(obj Json, key string, val string): Json", args: ["obj", "key", "val"] },
      { name: "set_int", sig: "set_int(obj Json, key string, val int): Json", args: ["obj", "key", "val"] },
      { name: "set_float", sig: "set_float(obj Json, key string, val float): Json", args: ["obj", "key", "val"] },
      { name: "set_bool", sig: "set_bool(obj Json, key string, val bool): Json", args: ["obj", "key", "val"] },
      { name: "add", sig: "add(arr Json, val Json): Json", args: ["arr", "val"] },
      { name: "add_str", sig: "add_str(arr Json, val string): Json", args: ["arr", "val"] },
      { name: "add_int", sig: "add_int(arr Json, val int): Json", args: ["arr", "val"] },
      { name: "encode", sig: "encode(v Json): string", args: ["v"] },
    ],
  },
  mem: {
    path: "std/mem",
    items: [
      { name: "scope", sig: "scope(): Arena", args: [] },
      { name: "release", sig: "release(a Arena)", args: ["a"] },
    ],
  },
};

function registerStdCompletion(context) {
  const provider = {
    provideCompletionItems(document, position) {
      const prefix = document.lineAt(position.line).text.slice(0, position.character);
      const member = prefix.match(/(?:^|[^A-Za-z0-9_.])([A-Za-z]+)\.[A-Za-z0-9_]*$/);
      if (member && STD_MODULES[member[1]]) {
        const mod = STD_MODULES[member[1]];
        return mod.items.map((it) => memberItem(document, member[1], mod, it));
      }
      // Not after a known module: offer the module names (auto-import on accept).
      return Object.keys(STD_MODULES).map((alias) => moduleItem(document, alias));
    },
  };
  context.subscriptions.push(
    languages.registerCompletionItemProvider("vader", provider, ".")
  );
}

function memberItem(document, alias, mod, it) {
  const item = new CompletionItem(it.name, CompletionItemKind.Function);
  item.detail = `${alias}.${it.sig}`;
  item.documentation = `Vader stdlib — import "${mod.path}"`;
  const params = it.args.map((a, i) => `\${${i + 1}:${a}}`).join(", ");
  item.insertText = new SnippetString(`${it.name}(${params})`);
  const edit = importEdit(document, mod.path);
  if (edit) item.additionalTextEdits = [edit];
  return item;
}

function moduleItem(document, alias) {
  const mod = STD_MODULES[alias];
  const item = new CompletionItem(alias, CompletionItemKind.Module);
  item.detail = `Vader stdlib — import "${mod.path}"`;
  item.documentation = "Inserts the import automatically when accepted.";
  const edit = importEdit(document, mod.path);
  if (edit) item.additionalTextEdits = [edit];
  return item;
}

// Returns a TextEdit inserting `import "<path>"` near the top, or undefined if
// the file already imports it. Placed above the first non-comment line so it
// coexists with both single and grouped imports.
function importEdit(document, path) {
  if (document.getText().includes(`"${path}"`)) return undefined;
  const lines = document.getText().split(/\r?\n/);
  let at = 0;
  while (at < lines.length && (/^\s*\/\//.test(lines[at]) || lines[at].trim() === "")) {
    at++;
  }
  const next = at < lines.length ? lines[at] : "";
  const spacer = next.trim() !== "" && !/^\s*import\b/.test(next) ? "\n" : "";
  return TextEdit.insert(new Position(at, 0), `import "${path}"\n${spacer}`);
}

function deactivate() {
  return client ? client.stop() : undefined;
}

module.exports = { activate, deactivate };
