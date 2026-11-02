## Summary

Describe the change briefly, including why it is needed and what it accomplishes.

## Related issues

Fixes #

## Type of change

- [ ] Bug fix
- [ ] New feature
- [ ] Performance improvement
- [ ] Refactoring
- [ ] Documentation

## Verification checklist

Run the following checks locally before submitting:

- [ ] `cargo test --workspace` passes without errors.
- [ ] `cargo clippy --workspace --all-targets` reports zero warnings.
- [ ] `cargo fmt --all --check` reports no formatting differences.
- [ ] `cargo +1.85 check --workspace --all-targets` confirms MSRV compatibility.
- [ ] Crate isolation is intact (`core`, `world`, and `input` remain free of `ash` and `winit`).
- [ ] Vulkan 1.0 rules are followed (no Vulkan 1.1+ calls or unprobed extensions).
- [ ] New or changed logic has unit test coverage where applicable.
