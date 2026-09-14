# Proposal: Fingerprint and similarity correctness

## Why
Audit found normalizer over/under-generalization and miscalibrated scoring: duration regex (`normalizer.rs:249`) collapses any `<num> s`; port rule (`:207`) misses `host:5432` while `line_column` (`:220`) mislabels `example.com:5432` as `<line>:<col>`; `temp_path` (`:189`) anchored `(^|\s)` misses `("/tmp/foo")`; `post_process` (`:114`) `split_whitespace().join(" ")` destroys Python/YAML indentation; `compile` (`:151`) `panic!`s on invalid regex, cache keyed by name only, `thread_local!` + `Box::leak` leaks; `score_file` (`score.rs:119`) requires exact token match so `<path>`-canonicalized paths never match and leaf `c.rs:10` never equals `c.rs`; `score_symbol` (`:98-101`) uses only the first canonical token so generic `error` matches alert symbol `error`; `tokenize.rs:32` drops `<3 char` tokens (`db,io,s3`) and has a tiny stopword list so generic `error/failed` dominate Jaccard; `score_tag` always `0.0` while `WEIGHTS=(0.5,0.2,0.2,0.1)` (`score.rs:27`) caps the effective max at `0.9` against threshold `0.65`.

## What Changes
- Tighten/loosen the four normalizer rules with regression fixtures; preserve indentation (normalize line-endings + trailing space only); fallible regex compilation with pattern-keyed cache and no leak.
- Recalibrate similarity: path-aware file scoring (placeholder-aware + leaf-name match), multi-token symbol scoring with IDF/stopwords, keep short domain tokens, implement or remove the tag weight, re-tune threshold against labeled fixtures.

## Capabilities
### New Capabilities
- `fingerprint-similarity`: corrected normalization + calibrated heuristic scoring.
### Modified Capabilities
- `fingerprinting-and-retention`: normalization rules.
- `correlation-and-ai-context`: scoring/threshold semantics.

## Impact
Affects: `src/fingerprint/normalizer.rs`, `src/similarity/tokenize.rs`, `src/similarity/score.rs`, `src/similarity/candidates.rs`, `tests/fingerprint.rs`, `tests/similarity.rs`, `tests/correlation.rs`.
