# Deployable project bootstrap

Copy these files into a deployable repository while preserving repository-
relative paths:

- `gate.toml` selects the mandatory `deployable` profile.
- `scripts/ci.sh` is the local CI contract and should be extended with the
  project's build and test commands.
- `compose.yaml` is the Compose contract starter.
- `.github/workflows/ci.yml` repeats the local command and `driftwatch gate`.

The template is scaffolding, not deployment evidence. Replace the sample
Compose service and add real project checks before treating the Gate as a
release or deployment signal. Configure remote branch protection separately
to require the CI workflow.
