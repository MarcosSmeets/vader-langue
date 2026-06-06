// Cliente da extensão Vader:
//  (1) liga o editor ao language server `vader lsp`;
//  (2) comandos de geração (struct/usecase/handler/fn) no menu de botão direito.
// JavaScript puro (sem build TS). Precisa de `npm install` uma vez (só pro LSP).

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
    return; // só realce + geração
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
      `Vader: não consegui iniciar o language server (${command} lsp). ` +
        `Ajuste "vader.serverPath" nas configurações. Detalhe: ${err.message}`
    );
  });
}

// --- (2) Geração de código (scaffolding) -------------------------------------

function registerGenCommands(context) {
  const gens = [
    ["vader.gen.struct", "struct", "struct"],
    ["vader.gen.usecase", "usecase", "use case"],
    ["vader.gen.handler", "handler", "handler"],
    ["vader.gen.fn", "fn", "função"],
  ];
  for (const [id, thing, label] of gens) {
    const disposable = commands.registerCommand(id, () => runGen(thing, label));
    context.subscriptions.push(disposable);
  }
}

async function runGen(thing, label) {
  const name = await window.showInputBox({
    prompt: `Nome do ${label} a gerar (vader gen ${thing} <Nome>)`,
    placeHolder: "Ex.: User",
    validateInput: (v) =>
      /^[A-Za-z_][A-Za-z0-9_]*$/.test(v.trim())
        ? null
        : "Use um identificador válido (letras, dígitos, _; não começa com dígito).",
  });
  if (!name) return;

  const bin = workspace.getConfiguration("vader").get("serverPath", "vader");
  // roda no terminal integrado pra usar o ambiente do usuário (PATH/WSL) e ver a saída.
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
