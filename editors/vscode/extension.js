// Vader extension client:
//  (1) connects the editor to the `vader lsp` language server;
//  (2) code-generation commands (struct/usecase/handler/fn) in the right-click menu.
// Plain JavaScript (no TS build). Needs `npm install` once (only for the LSP).

const { workspace, window, commands } = require("vscode");
const { LanguageClient, TransportKind } = require("vscode-languageclient/node");

let client;

function activate(context) {
  registerGenCommands(context);
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

function deactivate() {
  return client ? client.stop() : undefined;
}

module.exports = { activate, deactivate };
