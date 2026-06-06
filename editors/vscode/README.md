# Vader — extensão do VSCode

Duas coisas:
1. **Syntax highlighting** (`.vd`) — funciona em qualquer lugar, sem dependências.
2. **Language Server** — erros de parse e de tipo **em tempo real**, reusando o
   compilador (`vader lsp`). Precisa do binário `vader` e de um `npm install`.

## 1) Syntax highlighting (sem setup)

Realça comentários, palavras-chave, tipos (`int float bool string error chan map`),
strings, números, nomes de função e operadores (`<-`, `..`, `->`…). Mais `Ctrl+/` e
auto-fechamento de `{ [ ( "`.

Funciona assim que a extensão é carregada — veja "Como rodar" abaixo.

## 2) Language Server (erros em tempo real)

O servidor é o **próprio compilador**: `vader lsp` fala o Language Server Protocol por
stdio e publica diagnósticos com linha:coluna (os mesmos do `vader check`). O cliente
aqui só lança o processo — nada de reimplementar análise no editor.

Instale as dependências do cliente (uma vez):
```bash
cd editors/vscode
npm install
```

### ⚠️ WSL: o `vader` é um binário Linux

O toolchain da Vader é compilado no **WSL** (ELF Linux), então o VSCode do **Windows**
não roda o `vader` direto. Opções, da melhor pra mais simples:

- **Recomendado — VSCode + Remote-WSL:** abra o projeto dentro do WSL
  (`code .` de dentro do Ubuntu, ou "Reopen in WSL"). Aí a extensão roda no contexto
  Linux e enxerga o `vader`. Configure o caminho do binário se ele não estiver no PATH:
  ```jsonc
  // settings.json
  "vader.serverPath": "/mnt/c/Users/marco/Documents/workspace/side_projects/vader/target/debug/vader"
  ```
- **Só highlighting:** desligue o servidor e use apenas o realce:
  ```jsonc
  "vader.enableLanguageServer": false
  ```
- **Build nativo no Windows:** se um dia compilar um `vader.exe`, aponte
  `vader.serverPath` pra ele.

## Como rodar (modo dev)

1. Abra a pasta `editors/vscode` no VSCode (no contexto certo — veja acima).
2. `npm install` (só se quiser o language server).
3. Pressione **`F5`** → abre uma janela com a extensão carregada.
4. Abra um `.vd` (ex.: `examples/shapes.vd`). Realce aparece na hora; se o servidor
   estiver ligado, erros aparecem sublinhados enquanto você digita.

Ou instale localmente copiando a pasta pra `~/.vscode/extensions/vader-0.2.0` e reabrindo
o VSCode.

## Configurações

| Configuração | Padrão | O que faz |
|---|---|---|
| `vader.serverPath` | `vader` | Caminho do executável usado como `vader lsp`. |
| `vader.enableLanguageServer` | `true` | Liga/desliga os diagnósticos em tempo real. |
