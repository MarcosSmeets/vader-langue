// Vader extension client (plain JavaScript, no TS build):
//  (1) language server (`vader lsp`) for real-time diagnostics;
//  (2) code generation (struct/usecase/handler/fn) in the right-click menu;
//  (3) stdlib completion with automatic imports;
//  (4) run/build/test CodeLens + commands;
//  (5) formatting via `vader fmt`;
//  (6) hover, signature help, and an "add import" quick-fix;
//  (7) native Test Explorer for `test "..."` blocks.
// `npm install` is needed once (only for the LSP client dependency).

const {
  workspace, window, commands, languages, tests,
  CompletionItem, CompletionItemKind, SnippetString, TextEdit, WorkspaceEdit,
  Position, Range,
  Hover, MarkdownString, SignatureHelp, SignatureInformation, ParameterInformation,
  CodeAction, CodeActionKind, CodeLens, TestRunProfileKind, TestMessage,
} = require("vscode");
const { LanguageClient, TransportKind } = require("vscode-languageclient/node");
const cp = require("child_process");
const os = require("os");
const path = require("path");
const fs = require("fs");

let client;

function activate(context) {
  registerGenCommands(context);
  registerStdCompletion(context);
  registerRunCommands(context);
  registerFormatter(context);
  registerHoverAndSignature(context);
  registerImportQuickFix(context);
  registerTestController(context);
  startLanguageServer();
}

// --- shared helpers ----------------------------------------------------------

// The `vader` binary (also used as the LSP server). Configurable via vader.serverPath.
function binPath() {
  return workspace.getConfiguration("vader").get("serverPath", "vader");
}

function shellQuote(p) {
  return /\s/.test(p) ? `"${p}"` : p;
}

// Run a command line in the shared "Vader" integrated terminal (user's PATH/WSL env).
function runInTerminal(cmdline) {
  const term =
    window.terminals.find((t) => t.name === "Vader") ||
    window.createTerminal("Vader");
  term.show();
  term.sendText(cmdline);
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

// Global builtins (no module prefix) that still need a stdlib import.
const STD_GLOBALS = [
  { name: "newRouter", sig: "newRouter(): Router", args: [], path: "std/http" },
  { name: "serve", sig: "serve(port int, r Router)", args: ["port", "r"], path: "std/http" },
];

function registerStdCompletion(context) {
  const provider = {
    provideCompletionItems(document, position) {
      const prefix = document.lineAt(position.line).text.slice(0, position.character);
      const member = prefix.match(/(?:^|[^A-Za-z0-9_.])([A-Za-z]+)\.[A-Za-z0-9_]*$/);
      if (member && STD_MODULES[member[1]]) {
        const mod = STD_MODULES[member[1]];
        return mod.items.map((it) => memberItem(document, member[1], mod, it));
      }
      // Not after a known module: offer module names + global builtins (auto-import on accept).
      return [
        ...Object.keys(STD_MODULES).map((alias) => moduleItem(document, alias)),
        ...STD_GLOBALS.map((g) => globalItem(document, g)),
      ];
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

function globalItem(document, g) {
  const item = new CompletionItem(g.name, CompletionItemKind.Function);
  item.detail = g.sig;
  item.documentation = `Vader builtin — import "${g.path}"`;
  const params = g.args.map((a, i) => `\${${i + 1}:${a}}`).join(", ");
  item.insertText = new SnippetString(`${g.name}(${params})`);
  const edit = importEdit(document, g.path);
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

// --- (4) Run / build / test commands + CodeLens ------------------------------

function registerRunCommands(context) {
  const onActiveFile = (sub) => () => {
    const ed = window.activeTextEditor;
    if (!ed || ed.document.languageId !== "vader") {
      window.showWarningMessage("Vader: open a .vd file first.");
      return;
    }
    if (ed.document.isDirty) ed.document.save();
    runInTerminal(`${shellQuote(binPath())} ${sub} ${shellQuote(ed.document.fileName)}`);
  };

  context.subscriptions.push(
    commands.registerCommand("vader.runFile", onActiveFile("run")),
    commands.registerCommand("vader.buildFile", onActiveFile("build")),
    commands.registerCommand("vader.llvmFile", onActiveFile("llvm")),
    commands.registerCommand("vader.testFile", onActiveFile("test")),
    commands.registerCommand("vader.testProject", () => {
      const folder = workspace.workspaceFolders && workspace.workspaceFolders[0];
      const target = folder ? folder.uri.fsPath : ".";
      runInTerminal(`${shellQuote(binPath())} test ${shellQuote(target)}`);
    })
  );

  const codeLensProvider = {
    provideCodeLenses(document) {
      const lenses = [];
      const lines = document.getText().split(/\r?\n/);
      for (let i = 0; i < lines.length; i++) {
        const range = new Range(i, 0, i, 0);
        if (/^\s*(public\s+)?fn\s+main\s*\(\s*\)/.test(lines[i])) {
          lenses.push(new CodeLens(range, { title: "▶ Run", command: "vader.runFile" }));
          lenses.push(new CodeLens(range, { title: "Build", command: "vader.buildFile" }));
          lenses.push(new CodeLens(range, { title: "LLVM", command: "vader.llvmFile" }));
        } else if (/^\s*test\s+"(?:[^"\\]|\\.)*"\s*\{/.test(lines[i])) {
          lenses.push(new CodeLens(range, { title: "▶ Run tests", command: "vader.testFile" }));
        }
      }
      return lenses;
    },
  };
  context.subscriptions.push(
    languages.registerCodeLensProvider("vader", codeLensProvider)
  );
}

// --- (5) Formatting via `vader fmt` ------------------------------------------

function registerFormatter(context) {
  const provider = {
    provideDocumentFormattingEdits(document) {
      return new Promise((resolve) => {
        const tmp = path.join(os.tmpdir(), `vader-fmt-${process.pid}-${Date.now()}.vd`);
        try {
          fs.writeFileSync(tmp, document.getText());
        } catch (_e) {
          resolve([]);
          return;
        }
        cp.execFile(binPath(), ["fmt", tmp], { timeout: 10000 }, (err, stdout) => {
          fs.unlink(tmp, () => {});
          // On a syntax error (non-zero exit) or an unrunnable binary, leave the file untouched.
          if (err || !stdout) {
            resolve([]);
            return;
          }
          const full = new Range(
            document.positionAt(0),
            document.positionAt(document.getText().length)
          );
          resolve([TextEdit.replace(full, stdout)]);
        });
      });
    },
  };
  context.subscriptions.push(
    languages.registerDocumentFormattingEditProvider("vader", provider)
  );
}

// --- (6) Hover + signature help (reuses STD_MODULES / STD_GLOBALS) -----------

function registerHoverAndSignature(context) {
  const hover = {
    provideHover(document, position) {
      const wordRange = document.getWordRangeAtPosition(position, /[A-Za-z_][A-Za-z0-9_]*/);
      if (!wordRange) return null;
      const word = document.getText(wordRange);
      const before = document.getText(
        new Range(wordRange.start.line, 0, wordRange.start.line, wordRange.start.character)
      );
      const m = before.match(/([A-Za-z]+)\.\s*$/);
      if (m && STD_MODULES[m[1]]) {
        const it = STD_MODULES[m[1]].items.find((x) => x.name === word);
        if (it) return stdHover(`${m[1]}.${it.sig}`, STD_MODULES[m[1]].path, wordRange);
      }
      const g = STD_GLOBALS.find((x) => x.name === word);
      if (g) return stdHover(g.sig, g.path, wordRange);
      return null;
    },
  };

  const signature = {
    provideSignatureHelp(document, position) {
      const prefix = document.getText(new Range(position.line, 0, position.line, position.character));
      let it = null, label = null, argsSoFar = null;
      const mm = prefix.match(/([A-Za-z]+)\.([A-Za-z0-9_]+)\(([^()]*)$/);
      if (mm && STD_MODULES[mm[1]]) {
        it = STD_MODULES[mm[1]].items.find((x) => x.name === mm[2]);
        if (it) { label = `${mm[1]}.${it.sig}`; argsSoFar = mm[3]; }
      }
      if (!it) {
        const gm = prefix.match(/\b(newRouter|serve)\(([^()]*)$/);
        if (gm) { it = STD_GLOBALS.find((x) => x.name === gm[1]); if (it) { label = it.sig; argsSoFar = gm[2]; } }
      }
      if (!it) return null;
      const info = new SignatureInformation(label);
      info.parameters = (it.args || []).map((a) => new ParameterInformation(a));
      const help = new SignatureHelp();
      help.signatures = [info];
      help.activeSignature = 0;
      help.activeParameter = it.args && it.args.length
        ? Math.min((argsSoFar.match(/,/g) || []).length, it.args.length - 1)
        : 0;
      return help;
    },
  };

  context.subscriptions.push(
    languages.registerHoverProvider("vader", hover),
    languages.registerSignatureHelpProvider("vader", signature, "(", ",")
  );
}

function stdHover(sig, modPath, range) {
  const md = new MarkdownString();
  md.appendCodeblock(sig, "vader");
  md.appendMarkdown(`Vader stdlib — \`import "${modPath}"\``);
  return new Hover(md, range);
}

// --- (7) "Add import" quick-fix (reuses importEdit) --------------------------

function registerImportQuickFix(context) {
  const provider = {
    provideCodeActions(document, range) {
      const line = document.lineAt(range.start.line).text;
      const actions = [];
      const seen = new Set();
      let m;
      const re = /\b(db|http|json|mem)\./g;
      while ((m = re.exec(line))) {
        const mod = STD_MODULES[m[1]];
        if (mod && !seen.has(mod.path)) {
          seen.add(mod.path);
          const a = addImportAction(document, mod.path);
          if (a) actions.push(a);
        }
      }
      if (/\b(newRouter|serve)\b/.test(line) && !seen.has("std/http")) {
        const a = addImportAction(document, "std/http");
        if (a) actions.push(a);
      }
      return actions;
    },
  };
  context.subscriptions.push(
    languages.registerCodeActionsProvider("vader", provider, {
      providedCodeActionKinds: [CodeActionKind.QuickFix],
    })
  );
}

function addImportAction(document, modPath) {
  const edit = importEdit(document, modPath); // undefined if already imported
  if (!edit) return null;
  const action = new CodeAction(`Add import "${modPath}"`, CodeActionKind.QuickFix);
  action.edit = new WorkspaceEdit();
  action.edit.insert(document.uri, edit.range.start, edit.newText);
  return action;
}

// --- (8) Test Explorer for `test "..."` blocks -------------------------------

function registerTestController(context) {
  const ctrl = tests.createTestController("vader", "Vader");
  context.subscriptions.push(ctrl);

  const parseDoc = (document) => {
    if (document.languageId !== "vader" && !document.uri.fsPath.endsWith(".vd")) return;
    const uri = document.uri;
    const lines = document.getText().split(/\r?\n/);
    const children = [];
    for (let i = 0; i < lines.length; i++) {
      const m = lines[i].match(/^\s*test\s+"((?:[^"\\]|\\.)*)"\s*\{/);
      if (m) {
        const name = m[1].replace(/\\(.)/g, "$1");
        const item = ctrl.createTestItem(`${uri.toString()}::${name}`, name, uri);
        item.range = new Range(i, 0, i, lines[i].length);
        children.push(item);
      }
    }
    if (children.length === 0) {
      ctrl.items.delete(uri.toString());
      return;
    }
    const fileItem =
      ctrl.items.get(uri.toString()) ||
      ctrl.createTestItem(uri.toString(), workspace.asRelativePath(uri), uri);
    ctrl.items.add(fileItem);
    fileItem.children.replace(children);
  };

  workspace.textDocuments.forEach(parseDoc);
  workspace.findFiles("**/*.vd", "**/node_modules/**", 2000).then((uris) => {
    uris.forEach((u) => workspace.openTextDocument(u).then(parseDoc, () => {}));
  });
  context.subscriptions.push(
    workspace.onDidOpenTextDocument(parseDoc),
    workspace.onDidChangeTextDocument((e) => parseDoc(e.document)),
    workspace.onDidSaveTextDocument(parseDoc)
  );

  const runHandler = (request) => {
    const run = ctrl.createTestRun(request);
    const fileItems = [];
    const collect = (it) => {
      const f = it.parent || it;
      if (!fileItems.includes(f)) fileItems.push(f);
    };
    if (request.include) request.include.forEach(collect);
    else ctrl.items.forEach((f) => fileItems.push(f));

    const runFile = (fileItem) =>
      new Promise((resolve) => {
        fileItem.children.forEach((c) => run.started(c));
        cp.execFile(binPath(), ["test", fileItem.uri.fsPath], { timeout: 60000 }, (err, stdout, stderr) => {
          const out = (stdout || "") + (stderr || "");
          const results = {};
          out.split(/\r?\n/).forEach((ln) => {
            let mm = ln.match(/^\s*✓\s+(.+?)\s*$/);
            if (mm) { results[mm[1]] = true; return; }
            mm = ln.match(/^\s*✗\s+(.+?)\s*$/);
            if (mm) results[mm[1]] = false;
          });
          fileItem.children.forEach((c) => {
            if (results[c.label] === true) run.passed(c);
            else if (results[c.label] === false) run.failed(c, new TestMessage("Test failed"));
            else run.skipped(c);
          });
          if (Object.keys(results).length === 0 && out.trim()) {
            run.appendOutput(out.replace(/\r?\n/g, "\r\n"));
          }
          resolve();
        });
      });

    Promise.all(fileItems.map(runFile)).then(() => run.end());
  };

  ctrl.createRunProfile("Run", TestRunProfileKind.Run, runHandler, true);
}

function deactivate() {
  return client ? client.stop() : undefined;
}

module.exports = { activate, deactivate };
