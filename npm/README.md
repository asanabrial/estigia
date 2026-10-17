# @asanabrial/estigia

The npm route to [Estigia](https://github.com/asanabrial/estigia), workflow authority for AI coding
agents.

```sh
npx -y @asanabrial/estigia setup --all
```

This package is an installer, not a copy of the program. On first run, and whenever the installed
version differs from this package's (or from `ESTIGIA_VERSION`), it does what the release installers do: it downloads the release
archive for your platform, verifies it against the `.sha256` published beside it, records the
candidate with `estigia __record-install`, and places the binary in `~/.local/bin` (on Windows
`%LOCALAPPDATA%\estigia\bin`; `ESTIGIA_INSTALL_DIR` moves it). Then it runs that binary with the
arguments it was given.

The binary lives at that stable path rather than in npm's cache because `estigia setup` writes its own
path into every agent's hooks; a path that moves with each version would leave the hooks pointing at
nothing after an upgrade.

`ESTIGIA_VERSION` takes a tag other than this package's, and `ESTIGIA_BASE_URL` a mirror serving the
same layout. Everything this script prints goes to stderr, so `npx -y @asanabrial/estigia mcp` is a
clean MCP server on stdout.
