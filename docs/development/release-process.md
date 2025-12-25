# Release Process

This guide documents the release process for Oxigraph, including version numbering, checklists, and publishing to various package registries.

## Table of Contents

- [Version Numbering Scheme](#version-numbering-scheme)
- [Release Types](#release-types)
- [Release Checklist](#release-checklist)
- [Changelog Management](#changelog-management)
- [Publishing Process](#publishing-process)
- [Docker Images](#docker-images)
- [Post-Release Tasks](#post-release-tasks)

---

## Version Numbering Scheme

Oxigraph follows [Semantic Versioning 2.0.0](https://semver.org/):

```
MAJOR.MINOR.PATCH[-PRERELEASE][+BUILD]
```

### Version Components

- **MAJOR:** Incompatible API changes
- **MINOR:** Backward-compatible functionality additions
- **PATCH:** Backward-compatible bug fixes
- **PRERELEASE:** Optional pre-release identifier (alpha, beta, rc)
- **BUILD:** Optional build metadata

### Examples

```
0.4.0       - Normal release
0.4.1       - Patch release
0.5.0       - Minor release with new features
1.0.0       - First stable release
1.0.0-rc.1  - Release candidate
1.0.0-alpha - Alpha release
```

### Version Constraints

**Rust crates:**
- MSRV (Minimum Supported Rust Version): Currently 1.70.0
- Document in `Cargo.toml`: `rust-version = "1.70.0"`

**Python bindings:**
- Minimum Python version: 3.8+
- Document in `pyproject.toml`: `requires-python = ">=3.8"`

**JavaScript bindings:**
- Minimum Node.js version: 18+
- Document in `package.json`: `"engines": { "node": ">=18" }`

### Pre-1.0 Versions

Before 1.0.0, breaking changes are allowed in MINOR versions:
- `0.x.y`: MINOR version (x) may include breaking changes
- `0.x.y`: PATCH version (y) should be backward-compatible

### Post-1.0 Versions

After 1.0.0, strict semver applies:
- Breaking changes require MAJOR version bump
- New features require MINOR version bump
- Bug fixes require PATCH version bump

---

## Release Types

### 1. Patch Release (0.4.0 → 0.4.1)

**When to use:**
- Bug fixes only
- Security patches
- Documentation corrections
- Performance improvements without API changes

**Example scenarios:**
- Fixing a crash in SPARQL evaluation
- Correcting parsing error in RDF/XML
- Improving query performance
- Updating documentation typos

### 2. Minor Release (0.4.x → 0.5.0)

**When to use:**
- New features (backward-compatible)
- New SPARQL functions
- New RDF formats
- Deprecating APIs (with migration path)

**Example scenarios:**
- Adding GeoSPARQL support
- Supporting new RDF 1.2 features
- Adding new query optimization
- New JavaScript API methods

### 3. Major Release (0.x.y → 1.0.0)

**When to use:**
- Breaking API changes
- Removing deprecated features
- Major architectural changes
- Changing data storage format

**Example scenarios:**
- Rewriting storage engine
- Removing old deprecated APIs
- Changing function signatures
- First stable release (1.0.0)

### 4. Pre-Release

**Alpha (x.y.z-alpha):**
- Very early testing
- API may change significantly
- Not recommended for production

**Beta (x.y.z-beta.N):**
- Feature complete
- API mostly stable
- Needs testing and bug fixes

**Release Candidate (x.y.z-rc.N):**
- Final testing before release
- No new features
- Only critical bug fixes
- API frozen

---

## Release Checklist

### Pre-Release Preparation

#### 1. Code Freeze (1-2 weeks before)

- [ ] Create release branch: `git checkout -b release/v0.5.0`
- [ ] Announce code freeze to contributors
- [ ] Only accept critical bug fixes
- [ ] Finalize new features in progress

#### 2. Testing Phase

- [ ] Run full test suite: `cargo test --all`
- [ ] Run W3C compliance tests: `cargo test -p oxigraph --test testsuite`
- [ ] Test Python bindings: `cd python && python -m pytest`
- [ ] Test JavaScript bindings: `cd js && npm test`
- [ ] Run benchmarks: `cargo bench` (check for regressions)
- [ ] Test on all platforms (Linux, macOS, Windows)
- [ ] Test on all architectures (x86_64, aarch64)
- [ ] Fuzz testing: `cargo fuzz run sparql_parser -- -max_total_time=3600`

#### 3. Documentation Updates

- [ ] Update CHANGELOG.md with all changes
- [ ] Update version numbers in all `Cargo.toml` files
- [ ] Update version in `python/pyproject.toml`
- [ ] Update version in `js/package.json`
- [ ] Update version in `cli/Cargo.toml`
- [ ] Review and update README.md
- [ ] Update API documentation
- [ ] Check all code examples still work
- [ ] Update migration guide (if breaking changes)

#### 4. Version Bump

```bash
# Update versions in all Cargo.toml files
find . -name "Cargo.toml" -exec sed -i 's/version = "0.4.0"/version = "0.5.0"/g' {} \;

# Update Python version
sed -i 's/version = "0.4.0"/version = "0.5.0"/g' python/pyproject.toml

# Update JS version
cd js && npm version 0.5.0 --no-git-tag-version

# Verify changes
git diff
```

#### 5. Final Checks

- [ ] All CI checks pass
- [ ] No clippy warnings: `cargo clippy --all-targets -- -D warnings`
- [ ] Code is formatted: `cargo fmt --all -- --check`
- [ ] All dependencies up to date: `cargo update`
- [ ] Security audit: `cargo audit`
- [ ] License headers present in new files
- [ ] Copyright year updated (if new year)

---

## Changelog Management

### CHANGELOG.md Structure

Follow [Keep a Changelog](https://keepachangelog.com/) format:

```markdown
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- New features in development

### Changed
- Changes to existing functionality

### Deprecated
- Features to be removed in future releases

### Removed
- Features removed in this release

### Fixed
- Bug fixes

### Security
- Security fixes

## [0.5.0] - 2024-03-15

### Added
- GeoSPARQL support with spatial functions
- RDF 1.2 directional language tags
- New `bulk_loader()` API for faster imports

### Changed
- Improved SPARQL query optimization
- Updated RocksDB to 8.0

### Deprecated
- `Store::load()` in favor of `Store::bulk_load()`

### Fixed
- OPTIONAL pattern evaluation bug (#123)
- Memory leak in streaming parser (#145)

### Security
- Updated dependencies with security fixes

## [0.4.1] - 2024-02-10

### Fixed
- Critical bug in SPARQL UPDATE execution
- Parsing error for large literals

## [0.4.0] - 2024-01-15

...
```

### Changelog Best Practices

**Do:**
- Write for users, not developers
- Use simple language
- Link to issues/PRs: `(#123)`
- Group by category (Added, Changed, etc.)
- Include migration notes for breaking changes

**Don't:**
- Include internal refactoring unless user-facing
- List every commit
- Use technical jargon without explanation
- Forget to update [Unreleased] section

### Generating Changelog from Git

```bash
# List commits since last tag
git log v0.4.0..HEAD --oneline

# Group by type
git log v0.4.0..HEAD --oneline --grep="^feat:"
git log v0.4.0..HEAD --oneline --grep="^fix:"

# Use conventional commits for automation
# Tools: git-cliff, conventional-changelog
```

---

## Publishing Process

### 1. Create Git Tag

```bash
# Ensure you're on the correct branch
git checkout main

# Create annotated tag
git tag -a v0.5.0 -m "Release version 0.5.0"

# Push tag to GitHub (triggers CI)
git push origin v0.5.0
```

### 2. Publish to crates.io (Rust)

```bash
# Login (first time only)
cargo login

# Publish crates in dependency order
cd lib/oxsdatatypes && cargo publish
cd ../oxrdf && cargo publish
cd ../oxrdfxml && cargo publish
cd ../oxttl && cargo publish
cd ../oxjsonld && cargo publish
cd ../oxrdfio && cargo publish
cd ../spargebra && cargo publish
cd ../sparesults && cargo publish
cd ../sparopt && cargo publish
cd ../spareval && cargo publish
cd ../oxigraph && cargo publish

# Publish CLI
cd ../../cli && cargo publish
```

**Important:**
- Cannot unpublish from crates.io (can only yank)
- Must publish in dependency order
- Wait a few minutes between publishes for index to update
- Version numbers are immutable

**Verify publication:**
```bash
# Check on crates.io
open https://crates.io/crates/oxigraph

# Test installation
cargo install oxigraph-cli --version 0.5.0
```

### 3. Publish to PyPI (Python)

```bash
cd python

# Install publishing tools
pip install twine

# Build wheels (done by CI)
# Check artifacts from GitHub Actions

# Download wheels from CI
gh run download --name wheels

# Verify wheels
twine check dist/*.whl dist/*.tar.gz

# Upload to PyPI
twine upload dist/*

# Or upload to TestPyPI first
twine upload --repository testpypi dist/*
```

**CI Automated Process:**

GitHub Actions (`.github/workflows/artifacts.yml`) automatically:
1. Builds wheels for Linux (x86_64, aarch64)
2. Builds wheels for macOS (x86_64, arm64)
3. Builds wheels for Windows (x86_64)
4. Uploads to GitHub Release
5. (Optional) Auto-publishes to PyPI on release

**Verify publication:**
```bash
# Check on PyPI
open https://pypi.org/project/pyoxigraph/

# Test installation
pip install pyoxigraph==0.5.0
```

### 4. Publish to npm (JavaScript)

```bash
cd js

# Login (first time only)
npm login

# Build package
npm run build

# Verify package
npm pack
tar -xzf oxigraph-0.5.0.tgz
ls package/

# Publish
npm publish

# Or publish beta
npm publish --tag beta
```

**Verify publication:**
```bash
# Check on npm
open https://www.npmjs.com/package/oxigraph

# Test installation
npm install oxigraph@0.5.0
```

### 5. Create GitHub Release

**Option 1: Web Interface**

1. Go to https://github.com/oxigraph/oxigraph/releases
2. Click "Draft a new release"
3. Select tag: `v0.5.0`
4. Release title: `Oxigraph 0.5.0`
5. Copy changelog content
6. Attach binaries (from CI artifacts):
   - `oxigraph-server-linux-x86_64`
   - `oxigraph-server-linux-aarch64`
   - `oxigraph-server-macos-x86_64`
   - `oxigraph-server-macos-arm64`
   - `oxigraph-server-windows-x86_64.exe`
7. Click "Publish release"

**Option 2: GitHub CLI**

```bash
# Download release artifacts from CI
gh run download

# Create release
gh release create v0.5.0 \
  --title "Oxigraph 0.5.0" \
  --notes-file CHANGELOG_EXCERPT.md \
  oxigraph-server-*

# Mark as pre-release if needed
gh release create v0.5.0-rc.1 --prerelease
```

---

## Docker Images

### Build and Publish Docker Images

**Dockerfile location:** `cli/Dockerfile` or `server/Dockerfile`

#### Manual Build

```bash
# Build image
docker build -t oxigraph/oxigraph:0.5.0 -f cli/Dockerfile .

# Tag as latest
docker tag oxigraph/oxigraph:0.5.0 oxigraph/oxigraph:latest

# Push to Docker Hub
docker push oxigraph/oxigraph:0.5.0
docker push oxigraph/oxigraph:latest
```

#### Automated Build (GitHub Actions)

The release workflow automatically:
1. Builds multi-arch Docker images (linux/amd64, linux/arm64)
2. Pushes to Docker Hub
3. Tags as version and `latest`

**Configuration:** `.github/workflows/artifacts.yml`

```yaml
- name: Build and push Docker image
  uses: docker/build-push-action@v5
  with:
    context: .
    file: ./cli/Dockerfile
    platforms: linux/amd64,linux/arm64
    push: true
    tags: |
      oxigraph/oxigraph:${{ github.event.release.tag_name }}
      oxigraph/oxigraph:latest
```

**Verify Docker publication:**
```bash
# Check on Docker Hub
open https://hub.docker.com/r/oxigraph/oxigraph

# Test image
docker pull oxigraph/oxigraph:0.5.0
docker run oxigraph/oxigraph:0.5.0 --version
```

### Docker Image Variants

- `oxigraph/oxigraph:latest` - Latest release
- `oxigraph/oxigraph:0.5.0` - Specific version
- `oxigraph/oxigraph:0.5` - Minor version track
- `oxigraph/oxigraph:0` - Major version track
- `oxigraph/oxigraph:nightly` - Nightly builds

---

## Post-Release Tasks

### 1. Announcement

**Platforms to announce:**
- [ ] GitHub Release notes
- [ ] Twitter/X (@oxigraph)
- [ ] Semantic Web mailing list
- [ ] Gitter chat
- [ ] Update website (https://oxigraph.org)
- [ ] Blog post for major releases

**Announcement template:**

```markdown
# Oxigraph 0.5.0 Released

We're excited to announce Oxigraph 0.5.0!

## Highlights

- **GeoSPARQL Support:** Full support for spatial queries
- **RDF 1.2:** Directional language tags
- **Performance:** 30% faster query evaluation

## Installation

**Rust:**
```bash
cargo install oxigraph-cli
```

**Python:**
```bash
pip install pyoxigraph
```

**JavaScript:**
```bash
npm install oxigraph
```

**Docker:**
```bash
docker pull oxigraph/oxigraph:0.5.0
```

## Full Changelog

See the [full changelog](https://github.com/oxigraph/oxigraph/releases/tag/v0.5.0).

## Thank You

Thanks to all contributors who made this release possible!
```

### 2. Update Documentation

- [ ] Update docs.rs (automatic for Rust crates)
- [ ] Update https://oxigraph.org
- [ ] Update examples in documentation
- [ ] Add migration guide if breaking changes
- [ ] Update API reference

### 3. Monitor Release

**First 24 hours:**
- [ ] Monitor GitHub issues for bug reports
- [ ] Check installation works on all platforms
- [ ] Monitor download statistics
- [ ] Respond to questions in discussions

**First week:**
- [ ] Address critical bugs with patch release if needed
- [ ] Update documentation based on user feedback
- [ ] Plan next release

### 4. Update Project Board

- [ ] Close milestone for this release
- [ ] Create milestone for next release
- [ ] Move unfinished issues to next milestone
- [ ] Update project roadmap

### 5. Prepare for Next Release

```bash
# Create new version in CHANGELOG.md
echo "## [Unreleased]\n\n### Added\n\n### Changed\n\n### Fixed\n" >> CHANGELOG.md

# Bump version to next development version
# 0.5.0 -> 0.5.1-dev or 0.6.0-dev
```

---

## Emergency Procedures

### Yanking a Release

If a critical bug is discovered:

**Crates.io:**
```bash
# Yank the version (doesn't delete, prevents new usage)
cargo yank --version 0.5.0 oxigraph

# Undo yank if needed
cargo yank --undo --version 0.5.0 oxigraph
```

**PyPI:**
```bash
# Cannot delete, but can yank via web interface
# https://pypi.org/manage/project/pyoxigraph/releases/
```

**npm:**
```bash
# Unpublish within 72 hours
npm unpublish oxigraph@0.5.0

# Deprecate if past 72 hours
npm deprecate oxigraph@0.5.0 "Critical bug, use 0.5.1 instead"
```

### Hot Fix Release

For critical security issues:

1. **Create fix branch:**
   ```bash
   git checkout v0.5.0
   git checkout -b hotfix/v0.5.1
   ```

2. **Apply fix and test thoroughly**

3. **Fast-track release:**
   - Skip code freeze
   - Minimal changelog
   - Expedited review
   - Immediate publish

4. **Security advisory:**
   - Create GitHub Security Advisory
   - Assign CVE if applicable
   - Notify users

---

## Release Automation

### CI/CD Pipeline

The release is largely automated through GitHub Actions:

**On tag push (`v*`):**
1. Run all tests
2. Build release artifacts
3. Build Docker images
4. Create GitHub Release
5. Upload artifacts to release
6. Publish to package registries (if configured)

### Future Improvements

**Potential automations:**
- Automatic changelog generation from commits
- Automatic version bumping
- Auto-publish to crates.io/PyPI/npm on tag
- Automatic documentation deployment
- Release notes generation from issues

---

## Maintainer Notes

**Who can release:**
- Lead maintainer: @Tpt
- Authorized maintainers with publish rights

**Required permissions:**
- GitHub: Write access to repository
- crates.io: Owner or publisher rights
- PyPI: Maintainer role
- npm: Publisher role
- Docker Hub: Push permissions

**Security:**
- Use 2FA on all package registries
- Use API tokens instead of passwords
- Store tokens as GitHub Secrets for CI
- Rotate tokens periodically

---

## Additional Resources

- [Semantic Versioning](https://semver.org/)
- [Keep a Changelog](https://keepachangelog.com/)
- [Cargo Publishing Guide](https://doc.rust-lang.org/cargo/reference/publishing.html)
- [PyPI Publishing Guide](https://packaging.python.org/tutorials/packaging-projects/)
- [npm Publishing Guide](https://docs.npmjs.com/cli/v9/commands/npm-publish)
