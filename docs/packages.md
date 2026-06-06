# Vader — Pacotes e Dependências

> Criar libs para a Vader e instalá-las, estilo Cargo/npm — embutido no `vader`.
> Status: `draft v0.1`. Ver sistema de imports em [`grammar.md`](./grammar.md) §8.

---

## 1. Três origens de código

```
┌─ std/...            stdlib — vem com o compilador, ZERO install (inclui drivers de banco)
├─ <módulo local>     pacotes do seu próprio projeto (nome do módulo no vader.toml)
└─ <lib de terceiro>  publicada pela comunidade, instalada pelo gerenciador
```

## 2. Declaração no `vader.toml`

```toml
[project]
name    = "minha-api"
version = "0.1.0"
kind    = "api"

[dependencies]
http-extra = "1.2.0"                              # do registro central
auth-jwt   = { git = "github.com/fulano/auth-jwt", tag = "v0.3.1" }  # de git/URL
```

`vader.lock` trava as versões exatas resolvidas → build reprodutível.

## 3. Comandos

| Comando | Faz |
|---|---|
| `vader add <lib>[@versão]` | Adiciona dep e baixa (registro central) |
| `vader add <git-url>` | Adiciona dep direto de git/URL |
| `vader remove <lib>` | Remove dependência |
| `vader update [lib]` | Atualiza dentro das regras de semver |
| `vader install` | Restaura tudo a partir do `vader.lock` |
| `vader publish` | Publica sua lib no registro |

## 4. Distribuição: registro central **+** git/URL

- **Registro central** (padrão, estilo crates.io/npm): descoberta, busca, versionamento
  curado. É o caminho recomendado pra libs públicas.
- **git/URL** (escape hatch): instala direto de um repositório — ótimo pra libs privadas,
  forks ou algo ainda não publicado.

## 5. Publicar uma lib

Uma lib é um projeto `kind = "lib"` (arquitetura `minimal` por padrão).

```sh
vader new lib minha-lib
# ... escreve código + testes (cada função já nasce com seu *_test.vd) ...
vader test
vader publish            # versiona por semver e envia ao registro
```

Quem consome:
```sh
vader add minha-lib
```
```vader
import "minha-lib"

fn main() {
    minha-lib.doSomething()
}
```

## 6. Resolução de import

`import "x"` resolve nesta ordem: **stdlib** (`std/...`) → **pacote local** (prefixo =
nome do módulo) → **dependência** (registro/lock). Conflitos de nome são erro de
compilação — sem ambiguidade silenciosa.
