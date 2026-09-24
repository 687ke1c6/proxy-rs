# Directory upload for `client file` — implementation plan

Background: `client file <path>` currently sends exactly one file over exactly one
bidi stream on a fresh iroh connection (`run_send_file` in
[src/client/client.rs](../src/client/client.rs); server side is
`FileServerProtocolV1::handle()` in
[src/protocols/file_send/file_send_protocol_handler.rs](../src/protocols/file_send/file_send_protocol_handler.rs)).
This plan makes `<path>` accept a directory too: walk it recursively (not following
symlinks), and reproduce every regular file found under the chosen server volume,
preserving the tree's relative structure.

The good news: most of the hard part is already done. `FileSendHeader`'s
`target_dir` field is just an arbitrary `/`-separated relative path, and the server's
`safe_join` already `create_dir_all`s it (with the existing traversal guard) — so a
nested file at `sub/deeper/file.txt` is already fully expressible with today's wire
format, just by computing a longer `target_dir` per file. This is primarily a
client-side walk-and-loop feature, not a protocol redesign.

## Decisions (recommended defaults — confirm or override)

| Question | Recommendation |
|---|---|
| **Wire strategy for N files**: reuse one iroh connection with one bidi stream per file, or reconnect (fresh `endpoint.connect()` + ping) per file? | **One connection, many streams.** iroh connections are already QUIC-multiplexed — this is exactly what streams are for. Reconnecting per file means redoing the crypto handshake and ping preflight for every single file, which dominates for a directory with many small files. Requires a real (small) server change: `FileServerProtocolV1::handle()` currently does exactly one `connection.accept_bi()` then returns; it needs to loop, accepting streams until the client closes the connection. |
| **Destination nesting**: does the source directory's own name become a folder under `--target`, or do only its *contents* land there? | **Include the source dir's name** (`scp -r`/`cp -r` convention): `client file ./foo -t home/backup` with `foo/a.txt` and `foo/sub/b.txt` produces `<home>/backup/foo/a.txt` and `<home>/backup/foo/sub/b.txt`. Matches the mental model this tool already leans on (`tunnel` mode is explicitly "ssh `-L`-style"). The alternative (rsync's trailing-slash-means-contents-only convention) is easy to get backwards and harder to make obvious from the CLI alone. |
| **Conflict handling**: if a destination file already exists and `-o`/`--overwrite` wasn't passed, abort the whole directory transfer on the first conflict, or skip that file and keep going? | **Abort on first conflict**, same fail-fast behavior single-file mode already has. A partial-directory-transfer-with-a-summary-of-skips is a reasonable v2 idea but adds real complexity (partial success reporting, exit code semantics) for a case `-o` already solves cleanly. |
| **Symlinks** ("do not follow" per the request): skip entirely, or follow just the top-level `--file` argument if *it* happens to be a symlink? | **Skip every symlink encountered *during the walk*** (file or directory — a symlinked directory is not recursed into, a symlinked file is not sent), printing one line per skipped entry so nothing silently goes missing. The top-level `--file` argument itself is resolved with `std::fs::metadata` (which *does* follow), matching single-file mode's existing `std::fs::canonicalize` behavior today — the "don't follow" rule is about what the walk encounters underneath, not about refusing a symlink the user explicitly named. |

## Steps, in order

- [ ] **Add the `walkdir` crate** to [Cargo.toml](../Cargo.toml). It does not follow
      symlinked directories by default (matches the decision above for directories),
      but it *will* still yield a symlinked **file** as a normal entry — so the walk
      code must additionally check `entry.file_type().is_symlink()` and skip+warn on
      every symlink, not just rely on walkdir's default. Same "use a well-maintained
      crate instead of hand-rolling" call as `indicatif` — recursive directory
      walking with correct symlink handling is a solved problem.

- [ ] **Client: detect file vs. directory.** In `run_send_file` (or a new entry point
      it delegates to), after resolving `full_path`, branch on
      `tokio::fs::metadata(&full_path).await?.is_dir()`. The existing single-file
      body becomes the "it's a file" branch, unchanged in behavior.

- [ ] **Client: build a manifest for the directory case.**
  - [ ] `walkdir::WalkDir::new(&full_path)`, filtering out any entry whose
        `file_type().is_symlink()` is true (print a warning line per skip, e.g.
        `Skipping symlink: <path>`).
  - [ ] For each remaining regular file, compute:
    - its path relative to `full_path` (e.g. `sub/deeper/file.txt`)
    - its size (for the upfront total and per-file progress bar)
  - [ ] Collect into `Vec<{ absolute_path, relative_path, size }>`; sum total bytes
        and count. Print an upfront summary before sending anything, e.g.
        `Sending 42 files, 128.4 MiB total`.

- [ ] **Client: compute each file's `target_dir`/`file_name` for the wire header.**
      Per the nesting decision above: `target_dir = base_target_dir + "/" +
      <source-dir-basename> + "/" + relative_parent_dir_of_file`, `file_name =
      file's own basename`. (`base_target_dir` is exactly what `parse_target`
      already produces from `--target` today — no change needed there.)

- [ ] **Server: loop over multiple bidi streams per connection**
      ([file_send_protocol_handler.rs](../src/protocols/file_send/file_send_protocol_handler.rs)).
  - [ ] Wrap the current single-stream body of `handle()` in
        `loop { match connection.accept_bi().await { Ok(stream) => ..., Err(_) => break } }`
        — the loop ends naturally when the client closes the connection after its
        last file, no explicit "done" sentinel message needed.
  - [ ] **Important correctness fix while doing this**: `finish_and_close()` today
        calls both `send.finish()` *and* `connection.closed().await` after every
        single file. In the loop, only `send.finish()` should happen per file (to
        close that file's stream); `connection.closed().await` must move to *after*
        the loop exits, called once — otherwise the server would block waiting for
        the client to close the whole connection after file 1, deadlocking against a
        client that still has 41 more files to send on that same connection.
  - [ ] `resolve_destination`/`safe_join` need **zero changes** — they already take
        an arbitrary `target_dir` string and `create_dir_all` it with the existing
        traversal guard, which is exactly what nested-directory support needs.

- [ ] **Client: send the manifest over one connection.** Ping once, connect once
      (`FILE_ALPN_V1`), then loop the manifest: `conn.open_bi()` per file, run the
      existing header/ack/`copy_with_progress`/ack sequence (unchanged per-file
      logic, extracted into a small helper so both the single-file and directory
      paths call it), close the connection once after the loop.

- [ ] **Progress/UX**, building on the existing `copy_with_progress` bar:
  - [ ] Upfront summary line (file count + total size) before sending starts.
  - [ ] Per-file progress bar reused as-is (bytes/rate/ETA), with a `Sending file
        {i}/{n}: {relative_path}` line printed before each one starts.
  - [ ] Final summary after the loop: files sent, total bytes (`HumanBytes`), total
        elapsed, average rate, and a count of skipped symlinks if any were skipped.
  - [ ] Consider `indicatif::MultiProgress` (already available — no new dependency
        beyond `walkdir`) for an overall bar alongside the per-file bar, if a plain
        sequence of per-file bars feels cluttered in practice; not a hard
        requirement for v1.

- [ ] **Update help text and docs**: `cli.rs`'s `FileArgs.file` doc comment and
      [CLAUDE.md](../CLAUDE.md)'s `client file` line, to mention directory support
      and the symlink/conflict/nesting rules above.

- [ ] **Manual smoke test**:
  - [ ] Single file (regression) — confirm identical behavior/output to today.
  - [ ] Small nested tree (a few files, 2+ levels deep) — confirm structure and
        content match byte-for-byte on the server.
  - [ ] Tree containing a symlinked file — confirm it's skipped and warned about,
        not sent.
  - [ ] Tree containing a symlinked subdirectory — confirm it's skipped and warned
        about, not recursed into.
  - [ ] Re-running the same directory upload without `-o` — confirm it aborts
        cleanly on the first conflict, no partial mess.
  - [ ] Same, with `-o` — confirm it overwrites cleanly.
  - [ ] `client volumes` and single-file `client file` still work unmodified after
        the server-side loop change (regression check on the existing e2e-adjacent
        manual tests from prior plans).

## Explicitly out of scope for v1

- Preserving empty directories (one with zero files in it never triggers a
  `create_dir_all` today, since that only happens as a side effect of writing a
  file into it).
- Preserving file permissions/mtimes — single-file mode doesn't do this today
  either, so directory mode isn't a regression, just an existing limitation that
  now applies to more files at once.
- Concurrent/parallel file transfers (sequential is simpler to reason about for
  progress reporting and error handling; revisit if directory transfers turn out to
  be throughput-bound rather than latency-bound in practice).
- An opt-in flag to follow symlinks after all — add only if actually requested.
