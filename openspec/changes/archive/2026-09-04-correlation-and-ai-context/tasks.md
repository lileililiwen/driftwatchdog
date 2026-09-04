## 1. Similarity engine

- [x] 1.1 Define normalized correlation inputs, component scores, weights, threshold, and algorithm version.
- [x] 1.2 Write deterministic tests for token overlap, message/symbol/file/tag weighting, threshold filtering, and generic-text false-positive boundaries.
- [x] 1.3 Implement similarity and candidate generation without embeddings, network calls, or language-specific assumptions.

## 2. Persistence and manual links

- [x] 2.1 Persist correlations with bug/alert IDs, component scores, total score, algorithm version, and timestamps.
- [x] 2.2 Implement `link` validation and manual-link persistence with clear manual-vs-heuristic precedence.
- [x] 2.3 Implement `unlink` to remove exactly the selected link and report missing targets without destructive broad deletion.

## 3. AI report

- [x] 3.1 Write fixture tests for populated and empty `report --ai` output, including required sections and cautious wording.
- [x] 3.2 Implement AI-context Markdown rendering with stable IDs, recent evidence, scores, and investigation instructions.
- [x] 3.3 Verify report generation is offline, deterministic for the same state, and never invokes an LLM API.
