# Development Documentation

This directory contains comprehensive guides for Oxigraph contributors and maintainers.

## Available Guides

### For All Contributors

- **[Testing Guide](testing-guide.md)** - How to run tests, write tests, and ensure quality
  - Running tests locally (Rust, Python, JavaScript)
  - Test categories (unit, integration, W3C compliance)
  - Writing effective tests
  - Coverage requirements and tools
  - CI/CD test configuration

- **[Documentation Guide](documentation-guide.md)** - How to write and maintain documentation
  - Documentation standards and style
  - API documentation best practices
  - Writing tutorials and how-to guides
  - Code example requirements
  - Building docs locally

### For Maintainers

- **[Release Process](release-process.md)** - Complete release workflow
  - Version numbering scheme
  - Release checklist
  - Changelog management
  - Publishing to crates.io, PyPI, npm
  - Docker image builds

- **[API Stability Policy](api-stability.md)** - Stability guarantees and versioning
  - API stability guarantees
  - Deprecation policy
  - Breaking change process
  - Migration support
  - LTS versions (planned)

## Quick Links

### Getting Started

New to contributing? Start here:
1. Read the [CONTRIBUTING.md](../../docs/CONTRIBUTING.md)
2. Review the [Testing Guide](testing-guide.md)
3. Check the [Documentation Guide](documentation-guide.md)

### Common Tasks

- **Running tests:** See [Testing Guide - Running Tests Locally](testing-guide.md#running-tests-locally)
- **Writing docs:** See [Documentation Guide - Adding New Documentation](documentation-guide.md#adding-new-documentation)
- **Making a release:** See [Release Process - Release Checklist](release-process.md#release-checklist)
- **Breaking changes:** See [API Stability - Breaking Change Process](api-stability.md#breaking-change-process)

## Pull Request Templates

When creating pull requests, use the appropriate template:

- **[Documentation PR Template](../../.github/PULL_REQUEST_TEMPLATE/documentation.md)** - For documentation-only changes

More templates available in `.github/PULL_REQUEST_TEMPLATE/`

## Additional Resources

- [Project README](../../README.md)
- [CLAUDE.md](../../CLAUDE.md) - AI development guide
- [Main Documentation](../) - User-facing documentation
- [GitHub Workflows](../../.github/workflows/) - CI/CD configuration

## Contributing to These Docs

These development guides are living documents. If you find:
- Outdated information
- Missing procedures
- Unclear instructions
- Opportunities for improvement

Please open an issue or submit a PR to improve them!

---

**Questions?** Open a [GitHub Discussion](https://github.com/oxigraph/oxigraph/discussions) or ask in [Gitter](https://gitter.im/oxigraph/community).
