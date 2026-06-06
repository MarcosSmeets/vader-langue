# Vader — Packages and Dependencies

> Creating libs for Vader and installing them, Cargo/npm style — built into `vader`.
> Status: `draft v0.1`. See the imports system in [`grammar.md`](./grammar.md) §8.

---

## 1. Three sources of code

```
┌─ std/...            stdlib — comes with the compiler, ZERO install (includes database drivers)
├─ <local module>     packages from your own project (module name in vader.toml)
└─ <third-party lib>  published by the community, installed by the manager
```

## 2. Declaration in `vader.toml`

```toml
[project]
name    = "my-api"
version = "0.1.0"
kind    = "api"

[dependencies]
http-extra = "1.2.0"                              # from the central registry
auth-jwt   = { git = "github.com/someone/auth-jwt", tag = "v0.3.1" }  # from git/URL
```

`vader.lock` pins the exact resolved versions → reproducible build.

## 3. Commands

| Command | Does |
|---|---|
| `vader add <lib>[@version]` | Adds a dep and downloads it (central registry) |
| `vader add <git-url>` | Adds a dep straight from git/URL |
| `vader remove <lib>` | Removes a dependency |
| `vader update [lib]` | Updates within the semver rules |
| `vader install` | Restores everything from `vader.lock` |
| `vader publish` | Publishes your lib to the registry |

## 4. Distribution: central registry **+** git/URL

- **Central registry** (default, crates.io/npm style): discovery, search, curated
  versioning. It's the recommended path for public libs.
- **git/URL** (escape hatch): installs straight from a repository — great for private libs,
  forks, or something not yet published.

## 5. Publishing a lib

A lib is a `kind = "lib"` project (`minimal` architecture by default).

```sh
vader new lib my-lib
# ... write code + tests (each function is already born with its *_test.vd) ...
vader test
vader publish            # versions by semver and sends to the registry
```

Consumers:
```sh
vader add my-lib
```
```vader
import "my-lib"

fn main() {
    my-lib.doSomething()
}
```

## 6. Import resolution

`import "x"` resolves in this order: **stdlib** (`std/...`) → **local package** (prefix =
module name) → **dependency** (registry/lock). Name conflicts are a compile
error — no silent ambiguity.
