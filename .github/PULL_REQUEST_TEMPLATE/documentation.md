---
name: Documentation Pull Request
about: Use this template for documentation-only changes
title: 'docs: '
labels: documentation
---

## Documentation Changes

### Type of Documentation

<!-- Check all that apply -->

- [ ] API documentation (code comments/docstrings)
- [ ] Tutorial
- [ ] How-to guide
- [ ] Reference documentation
- [ ] Explanatory documentation
- [ ] README or getting started guide
- [ ] Migration guide
- [ ] Other (please specify):

### What is being added or changed?

<!-- Provide a brief description of the documentation changes -->



### Motivation

<!-- Why is this documentation needed? What problem does it solve? -->



### Related Issues

<!-- Link any related issues or discussions -->

Fixes #
Related to #

---

## Documentation Quality Checklist

### Content Quality

- [ ] **Accuracy:** All information is correct and up-to-date
- [ ] **Completeness:** All important aspects are covered
- [ ] **Clarity:** Easy to understand for the target audience
- [ ] **Conciseness:** No unnecessary information or verbosity
- [ ] **Grammar:** Proper grammar, spelling, and punctuation
- [ ] **Consistency:** Follows existing documentation style and terminology

### Code Examples

- [ ] **All code examples compile and run successfully**
- [ ] **Examples include necessary imports**
- [ ] **Error handling is demonstrated appropriately**
- [ ] **Examples are self-contained and realistic**
- [ ] **Doc tests pass:** `cargo test --doc` (Rust) or equivalent
- [ ] **Examples follow best practices**

### Structure and Formatting

- [ ] **Proper markdown formatting** (headers, lists, code blocks)
- [ ] **Table of contents** included for long documents
- [ ] **Section headings** are descriptive and properly nested
- [ ] **Links** are working and point to correct targets
- [ ] **Images/diagrams** included if helpful (with alt text)

### Navigation and References

- [ ] **Links to related documentation** provided
- [ ] **Cross-references** are accurate
- [ ] **External links** are valid and appropriate
- [ ] **Section anchors** work correctly

### Testing

- [ ] **Rust doc tests pass:** `cargo test --doc --all`
- [ ] **Python doctests pass:** `python -m pytest --doctest-modules`
- [ ] **JavaScript examples validated**
- [ ] **Manual testing** of procedures/instructions completed

### Maintenance

- [ ] **Version numbers** are current
- [ ] **Deprecated features** are marked appropriately
- [ ] **Migration notes** included for breaking changes (if applicable)
- [ ] **Changelog** updated (if applicable)

---

## Review Criteria

### For Reviewers

Please verify:

1. **Technical Accuracy:**
   - [ ] Code examples work correctly
   - [ ] API usage is correct
   - [ ] Descriptions match actual behavior

2. **Clarity and Readability:**
   - [ ] Easy to understand for target audience
   - [ ] Well-organized and logical flow
   - [ ] No ambiguous or confusing language

3. **Completeness:**
   - [ ] All necessary information provided
   - [ ] Edge cases and errors documented
   - [ ] Links to related resources included

4. **Style and Formatting:**
   - [ ] Consistent with existing docs
   - [ ] Proper markdown syntax
   - [ ] Good visual presentation

---

## Preview

<!-- If possible, provide a preview of the rendered documentation -->
<!-- Screenshots, links to rendered markdown, or formatted text -->

### Before

<!-- What did the documentation look like before (if updating existing docs)? -->



### After

<!-- What will the documentation look like after this PR? -->



---

## Additional Context

<!-- Any additional information that would be helpful for reviewers -->



---

## Deployment Notes

<!-- For maintainers: Any special steps needed to deploy these docs? -->

- [ ] Requires website rebuild
- [ ] Requires docs.rs update
- [ ] Requires README sync
- [ ] Other:

---

## Checklist for Submitter

Before requesting review:

- [ ] I have read the [Documentation Guide](../../docs/development/documentation-guide.md)
- [ ] All code examples have been tested
- [ ] Doc tests pass locally
- [ ] Markdown is properly formatted
- [ ] Links have been verified
- [ ] Spelling and grammar checked
- [ ] Screenshots are up-to-date (if applicable)
- [ ] Related documentation has been updated
- [ ] CHANGELOG.md updated (if applicable)

---

<!--
Thank you for improving Oxigraph's documentation!

Good documentation helps everyone in the community.
Your contribution is greatly appreciated.
-->
