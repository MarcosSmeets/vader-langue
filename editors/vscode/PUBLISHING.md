# Publishing the Vader extension to the VS Code Marketplace

In short: you create a **publisher** (via Azure DevOps), generate a **token (PAT)**,
package with `vsce`, and publish. It takes ~15 minutes the first time.

## 0. Prerequisites
- Node.js installed.
- The official tool: `npm install -g @vscode/vsce`
- (Optional, for VSCodium/Open VSX) `npm install -g ovsx`

## 1. Create an organization on Azure DevOps
1. Go to https://dev.azure.com with a Microsoft account (free to create).
2. Create any organization (the name doesn't matter for the marketplace).

## 2. Generate a Personal Access Token (PAT)
1. In Azure DevOps: top-right → **User settings** → **Personal access tokens**.
2. **New Token**:
   - Organization: **All accessible organizations** (important!).
   - Scopes: **Custom defined** → check **Marketplace → Manage**.
   - Expiration: whatever you prefer.
3. **Copy the token** (it's shown only once).

## 3. Create the publisher
1. Go to https://marketplace.visualstudio.com/manage
2. **Create publisher**: pick an **ID** — it's unique and public. This project
   already uses `Vader` (the `publisher` field in `package.json`).
3. If you change the ID, update `publisher` in this folder's `package.json`, and
   make sure `repository`/`bugs` point at your real GitHub.

## 4. Package and test locally
```bash
cd editors/vscode
npm install
vsce package          # generates vader-0.5.0.vsix
```
Test the `.vsix` before publishing:
```bash
code --install-extension vader-0.5.0.vsix
```

## 5. Publish
```bash
vsce login Vader      # paste the PAT when prompted
vsce publish          # publishes the version in package.json
```
Or in a single command:
```bash
vsce publish -p <YOUR_PAT>
```
To bump the version automatically: `vsce publish minor` (or `patch`/`major`).
**Note:** the version in `package.json` is already set, so plain `vsce publish`
publishes it as-is — don't add `minor`/`patch` unless you want a further bump.

The extension shows up at https://marketplace.visualstudio.com/items?itemName=Vader.vader
within a few minutes.

## 6. (Optional) Open VSX — for VSCodium / Gitpod / Theia
```bash
ovsx publish vader-0.5.0.vsix -p <OPEN_VSX_TOKEN>
```
Get a token at https://open-vsx.org (GitHub login → Access Tokens).

## Pre-publish checklist
- [x] `publisher` set (`Vader`).
- [x] `repository`/`bugs` point to GitHub.
- [ ] `version` correct in `package.json`.
- [x] `README.md` and `LICENSE` present.
- [x] `icon.png` (128×128) and `"icon"` set in `package.json`.
- [ ] `vsce package` runs without warnings that bother you.

## Important note (LSP)
The extension gives syntax highlighting to everyone, but **real-time diagnostics**
depend on the `vader` binary on the user's machine (it runs `vader lsp`). If the
user doesn't have `vader`, highlighting still works and the server simply doesn't
start (the extension already handles this and offers the
`vader.enableLanguageServer: false` option). Make that clear in the extension's
marketplace description.
