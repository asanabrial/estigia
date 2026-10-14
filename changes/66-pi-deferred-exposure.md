section: The harness

- **On Pi 1.2.0 and later, Estigia's MCP server is written `deferred`, not `direct`.** Issue #66. `estigia setup pi` reads
  the version from `pi --version`, writes `exposure: "deferred"` and a `description` for a Pi that lists deferred
  servers and loads their tools on demand, and keeps `direct` exactly as before for an older Pi or one whose version
  cannot be read. `--dry-run` prints the exposure it would write and why, and `doctor`'s `pi-config` row names the
  exposure and the Pi version and warns when a Pi at 1.2.0 or later still carries `direct`. A project `.pi/mcp.json`
  override of `deferred` is no longer a finding. The other adapters' MCP entries are unchanged.
