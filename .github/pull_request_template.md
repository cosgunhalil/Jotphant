## What does this change?

Describe the change and why it is needed. Link the related issue if one exists.

## Checklist

- [ ] Commit messages follow Conventional Commits. They drive our release
      versions and changelog, see RELEASING.md.
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --all-targets` passes with zero warnings
- [ ] `cargo test` passes
- [ ] New user-visible strings are added to every catalog under `locales/`.
      The completeness test will fail otherwise.
- [ ] Docs are updated where behavior changed: README, SCOPE.md, CONTRIBUTING.md
