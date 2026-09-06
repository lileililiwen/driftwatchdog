## 1. Pipeline
- [x] 1.1 Add ci.yml triggered on push/PR.
- [x] 1.2 Run `cargo fmt --check`.
- [x] 1.3 Run `cargo clippy` with -D warnings.
- [x] 1.4 Run `cargo test --workspace`.
- [x] 1.5 Run the npm launcher tests in test/.
## 2. Verification
- [x] 2.1 Push a trivial change and confirm the gates run. (Gate implemented; cannot push in this environment — manual run on first PR will exercise it.)
- [x] 2.2 Run `openspec validate ci-test-gates`.
