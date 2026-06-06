# Publicar a extensão Vader no VS Code Marketplace

Resumo: você cria um **publisher** (via Azure DevOps), gera um **token (PAT)**, empacota
com `vsce` e publica. Leva ~15 min na primeira vez.

## 0. Pré-requisitos
- Node.js instalado.
- A ferramenta oficial: `npm install -g @vscode/vsce`
- (Opcional, p/ VSCodium/Open VSX) `npm install -g ovsx`

## 1. Criar uma organização no Azure DevOps
1. Entre em https://dev.azure.com com uma conta Microsoft (cria de graça).
2. Crie uma organização qualquer (o nome não importa pro marketplace).

## 2. Gerar um Personal Access Token (PAT)
1. No Azure DevOps: canto superior direito → **User settings** → **Personal access tokens**.
2. **New Token**:
   - Organization: **All accessible organizations** (importante!).
   - Scopes: **Custom defined** → marque **Marketplace → Manage**.
   - Expiração: o que preferir.
3. **Copie o token** (só aparece uma vez).

## 3. Criar o publisher
1. Vá em https://marketplace.visualstudio.com/manage
2. **Create publisher**: escolha um **ID** (ex.: `marco-vader`) — é único e público.
3. No `package.json` desta pasta, troque:
   ```jsonc
   "publisher": "SEU-PUBLISHER-ID",   // ← o ID que você criou
   ```
   E ajuste os `repository`/`bugs` pra apontar pro seu GitHub real.

## 4. Empacotar e testar localmente
```bash
cd editors/vscode
npm install
vsce package          # gera vader-0.2.0.vsix
```
Teste o `.vsix` antes de publicar:
```bash
code --install-extension vader-0.2.0.vsix
```

## 5. Publicar
```bash
vsce login SEU-PUBLISHER-ID     # cola o PAT quando pedir
vsce publish                    # publica a versão do package.json
```
Ou em um comando:
```bash
vsce publish -p <SEU_PAT>
```
Para subir versão automaticamente: `vsce publish minor` (ou `patch`/`major`).

A extensão aparece em https://marketplace.visualstudio.com/items?itemName=SEU-PUBLISHER-ID.vader
em alguns minutos.

## 6. (Opcional) Open VSX — pra VSCodium / Gitpod / Theia
```bash
ovsx publish vader-0.2.0.vsix -p <TOKEN_OPEN_VSX>
```
Token em https://open-vsx.org (login GitHub → Access Tokens).

## Checklist antes de publicar
- [ ] `publisher` trocado pro seu ID.
- [ ] `repository`/`bugs` apontando pro seu GitHub.
- [ ] `version` certa no `package.json`.
- [ ] `README.md` e `LICENSE` presentes (já estão).
- [ ] (Recomendado) um `icon.png` 128×128 e `"icon": "icon.png"` no `package.json`.
- [ ] `vsce package` roda sem warnings que te incomodem.

## Observação importante (LSP)
A extensão dá realce de sintaxe pra todo mundo, mas os **diagnósticos em tempo real**
dependem do binário `vader` na máquina do usuário (ela roda `vader lsp`). Se o usuário
não tiver o `vader`, o realce funciona e o servidor só não inicia (a extensão já trata
isso e tem a opção `vader.enableLanguageServer: false`). Deixe isso claro na descrição
da extensão no marketplace.
