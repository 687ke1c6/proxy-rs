# Code Review Findings — node-id naming (HEAD~1 diff)

Review of `git diff HEAD~1 -- CLAUDE.md src/client/client.rs src/client/client_helpers.rs` (adds custom naming to the interactive `<new>` node-id flow). Ordered most severe first — work top to bottom.

## Correctness

- [ ] **Duplicate-ID save silently drops the user's typed name**
  `src/client/client_helpers.rs:122` — `write_node_id_to_file`'s existing-key dedupe check returns early *before* it ever looks at the `name` argument. If a user re-enters a node ID that's already saved and types a real name (e.g. "Prod Server") at the new Name prompt, the entry keeps its old name and the typed name is thrown away with zero feedback.

- [ ] **Name prompt accepts whitespace-only input → empty name persisted**
  `src/client/client_helpers.rs:110` — unlike the ID prompt just above it, the new Name `Input` has no `.validate_with(...)`. Typing a single space bypasses `.default(generate_name())` and survives `.trim().to_string()` as `""`, which gets written to `node-ids.yaml`. The next run's Select menu then shows a blank label like `" [abc1234...]"`.

- [ ] **No uniqueness check on node entry names**
  `src/client/client_helpers.rs:116` — only `key` is deduped; nothing stops two different node IDs from sharing the same name. Since users now actively choose names (rather than always getting a fresh random one), collisions are much more reachable than before this diff, making the Select menu ambiguous.

## Documentation

- [ ] **CLAUDE.md overstates equivalence between `-n` and `<new>`**
  `CLAUDE.md:56` — claims the interactive flow "saves both exactly as if the ID had been passed via `-n`." Not true anymore: `-n` (`client.rs:117`) always calls `write_node_id_to_file(id, None)`, forcing an auto-generated name, while `<new>` saves the user's typed/confirmed name. Update the wording to reflect that only the persistence *mechanism* is shared, not the resulting name.

## Consistency

- [ ] **`run_send_file`'s `-n` path never persists the node ID at all**
  `src/client/client.rs:117` (compare with `run_send_file`) — pre-existing gap, not introduced by this diff, but the diff touched the sibling `run_tcp_client` call site and left this asymmetry unaddressed. A user who only ever runs `proxy-rs -f <path> -n <id>` never gets that ID saved to `node-ids.yaml`.

## Efficiency

- [ ] **Redundant reads/writes when adding a new node entry**
  `src/client/client_helpers.rs:57` — persisting one new entry and marking it `last_used` costs 3 reads + 2 writes of `node-ids.yaml` (load in `load_node_id_from_file`, read+write in `write_node_id_to_file`, read+write again back in `load_node_id_from_file`). Could be 1 read + 1 write by reusing the already-loaded config.

## Simplification

- [ ] **`Option<&str>` forces an avoidable clone**
  `src/client/client_helpers.rs:126` — `write_node_id_to_file` takes `name: Option<&str>` and immediately does `.map(str::to_string)`, even though the only caller passing `Some` already owns a fresh `String` it never reuses. `Option<String>` would drop the extra clone/indirection.

- [ ] **`prompt_new_node_id` returns an unlabeled `(String, String)` tuple**
  `src/client/client_helpers.rs:117` — returns `Result<(String, String)>` positionally for `(id, name)`, even though `NodeEntry { name, key }` already exists in the same file and is shaped identically. Returning `NodeEntry` instead removes the swap risk and self-documents the fields at call sites.
