## Approach
- Duration: require a duration cue (time/duration/elapsed/timeout/latency context) or a known unit list (`ms|s|sec|secs|second(s)|m|min(s)|h|hr(s)`) adjacent to timing words — never bare `<num> s`.
- Port vs line:col: match `:port` for known service hosts/patterns first (`host|port|localhost|127.0.0.1|postgres|mysql|redis|mongo|example.com`-generic `host(:port)`), and require `path.rs:<line>(:<col>)?` shape for line:col (file-like prefix with extension or `/`).
- Temp path: match after `(`, `"`, `'`, `=`, `:` as well as whitespace.
- Whitespace: `lines().map(trim_end).join("\n")`, collapse `\r\n`, keep leading indentation; update golden fingerprints + add Python-traceback fixture.
- Regex: `try_compile` returning `Result`, cache key `(name, pattern)`, `OnceLock<Vec<Regex>>` instead of `Box::leak`.
- Scoring: `score_file` matches `<path>` placeholder against any path token + leaf equality (`c.rs:10` ~ `c.rs` partial credit); `score_symbol` over all tokens with stopword/IDF down-weight; keep allowlist short tokens (`db,io,os,s3,api,ui`); either implement `score_tag` or set its weight to 0 and document; re-tune threshold on a checked-in labeled fixture set (target: no generic-`error` false positive, absolute-path true positive passes).

## Non-goals
- No ML embeddings / vector DB (v1 non-goal); heuristic text similarity only.
