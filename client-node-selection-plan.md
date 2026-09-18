# Client node-id/name resolution — implementation plan

Scope: `-l` (client/TCP-proxy mode, `run_tcp_client`) only. `-f` (file-send mode) and
server mode are untouched by this plan — they keep today's behavior (raw id or the
existing interactive `Select`).

## Target behavior (client mode, `-l`)

| Flags | Behavior |
|---|---|
| `-n <id> --name 'Bob'` | If `<id>` is already saved, use it and rename that entry to `Bob`. If not saved, add it under the name `Bob`. Continue either way. |
| `-n <id>` | If `<id>` is already saved, use it as-is (no rename). If not saved, add it with a generated name. Continue. |
| `--name 'Bob'` (no `-n`) | If an entry named `Bob` exists, use its node id. If not, prompt (TUI) for just a node id, save it as `Bob`, continue. |
| neither flag | Unchanged: today's interactive `Select` (existing entries, recency-sorted, + `<new>`). |

## Steps

- [x] **CLI**: add `--name` (long-only, no short flag — `-n`/`-l`/`-f`/`-o`/`-d` are
      already taken) to `Args` in [src/main.rs](src/main.rs), with `env = "PROXY_RS_NAME"`
      for consistency with the other flags. Pass it through to `run_tcp_client` only;
      leave `run_send_file`'s call site unchanged.

- [x] **`client_helpers.rs` data-access helpers**: add small helpers operating on
      `ClientConfig`/`NodeEntry` so the resolver (next step) doesn't hand-roll vector
      scans:
  - [x] `find_by_key(&ClientConfig, &str) -> Option<usize>`
  - [x] `find_by_name(&ClientConfig, &str) -> Option<usize>`
  - [x] a rename op (update `node_entries[i].name` in place + save)
  - [x] a "prompt for node id only" helper — reuses the same `EndpointId::from_str`
        validation as `prompt_new_node_id`, but skips its trailing name prompt (the
        name is already known in the case that needs this: `--name` with no match).

- [x] **New resolver**: `resolve_node_id(node_id: Option<String>, name: Option<String>) -> Result<String>`
      in `client_helpers.rs`, implementing the four cases in the table above:
  - [x] `(Some(id), Some(name))` → find by key; found → rename entry to `name`, save,
        return `id`. Not found → append `{ name, key: id }`, save, return `id`.
  - [x] `(Some(id), None)` → find by key; found → return `id` unchanged. Not found →
        append with a generated name (today's `write_node_id_to_file` behavior),
        return `id`.
  - [x] `(None, Some(name))` → find by name; found → return its `key`. Not found →
        run the id-only prompt, append `{ name, key: prompted_id }`, save, return
        `prompted_id`.
  - [x] `(None, None)` → delegate to the existing `load_node_id_from_file()`
        interactive flow, unchanged.
  - [x] In every branch that resolves to a concrete id, update `last_used` (today
        only the interactive path does this) so recency-sorting in the `Select` menu
        stays meaningful regardless of how an id was last resolved.

- [x] **Wire `client.rs`**: change `run_tcp_client`'s signature to take
      `name: Option<String>` alongside `server_node_id_str: Option<String>`, and
      replace the current

      ```rust
      if let Some(id) = &server_node_id_str {
          write_node_id_to_file(id, None)?;
      }
      let raw = match server_node_id_str { Some(id) => id, None => load_node_id_from_file()? };
      ```

      with a single call to `resolve_node_id(server_node_id_str, name)`.

- [x] **Edge cases** (decided with user, implemented):
  - [x] Duplicate names: `--name` alone matching more than one saved entry → use the
        first match and log a `warn!` that the name is ambiguous.
  - [x] `-n <id> --name 'Bob'` where `'Bob'` is already the name of a *different*
        existing entry → hard error (`anyhow::bail!`), telling the user to pick a
        different `--name` rather than silently creating/allowing duplicate names.
  - [x] `--name`/`-n` passed together with `-f` (file mode) or with no mode flags at
        all (server mode) → ignored silently; `args.name` is only ever threaded
        through to `run_tcp_client`, so no extra code was needed for this.

- [ ] **Docs**: update the "Client flow" section of [CLAUDE.md](CLAUDE.md) — it
      currently documents the old "`-n` always skips the menu, no way to name a
      fresh entry" behavior and needs to describe `--name`'s three new cases.

- [ ] **Testing**:
  - [ ] Manual smoke test of all four flag combinations against a scratch
        `--config-dir`, checking `node-ids.yaml` contents after each.
  - [ ] Confirm `tests/e2e/socks5_proxy.sh` still passes unmodified (it drives the
        client via `-n`, which keeps its existing "use if known, else add with a
        generated name" behavior).
