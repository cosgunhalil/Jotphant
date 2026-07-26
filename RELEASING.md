# Releasing Jotphant

How versions are numbered and how a release ships. The pipeline has three parts:

| File | Job |
|---|---|
| `release-plz.toml` + `.github/workflows/release-plz.yml` | Compute the next version from commits, maintain `CHANGELOG.md`, keep the **Release PR** open, tag on merge |
| `.github/workflows/release.yml` | On a `v*` tag: test, build the Windows binary, zip, publish the GitHub Release |
| `.github/workflows/ci.yml` | fmt + clippy + tests on every push/PR |

## Versioning rules

- **SemVer** `MAJOR.MINOR.PATCH`; the git tag is `v{version}` and MUST equal
  `Cargo.toml`'s `version`.
- The bump is decided by the **Conventional Commit types since the last release**
  (highest wins). After 1.0:
  - any commit with `!` or `BREAKING CHANGE:` → **major**. For an application,
    "breaking" means the user's world breaks: a database change that cannot
    auto-migrate, an incompatible config format, a removed feature.
  - any `feat:` → **minor**
  - only `fix:` / `perf:` / `chore:` / `docs:` / `ci:` / `test:` → **patch**
- Before 1.0, semver treats the minor digit as the breaking slot, so bumps are
  more conservative. The Release PR always shows the computed version — **review
  it there, and edit the PR if a different bump is warranted.**
- `1.0.0` is a deliberate, human decision: the app is stable in daily use and we
  stand behind its data compatibility.
- Never move, reuse, or delete a published tag. A bad release is fixed by a new
  patch release.

## Day-to-day flow

1. Work merges into `master` with Conventional Commit messages, as always.
2. release-plz keeps a **Release PR** open ("chore: release vX.Y.Z") containing
   the version bump in `Cargo.toml` and the changelog for everything since the
   last release.
3. **Ship = merge that PR.** The `v*` tag is created automatically, and
   `release.yml` tests, builds, and publishes the GitHub Release with the
   Windows zip attached.

Release cadence is yours: merge the Release PR after every fix, or let features
accumulate — the version is computed either way.

## One-time setup: the `RELEASE_PLZ_TOKEN` secret

GitHub does not let a workflow's default `GITHUB_TOKEN` trigger other workflows,
so a tag created with it would never fire `release.yml`. release-plz therefore
needs a personal access token:

1. GitHub → Settings → Developer settings → **Fine-grained personal access
   tokens** → Generate new token.
2. Scope it to this repository with permissions: **Contents: Read and write**,
   **Pull requests: Read and write**.
3. Repository → Settings → Secrets and variables → Actions → **New repository
   secret**, name it `RELEASE_PLZ_TOKEN`, paste the token.

## Manual fallback

The automated path is optional; a hand-cut release always works:

```bash
git tag v0.2.0
```

```bash
git push origin v0.2.0
```

Bump `Cargo.toml`'s `version` (commit as `chore(release): v0.2.0`) *before*
tagging so the tag and manifest agree. `release.yml` takes it from there.
