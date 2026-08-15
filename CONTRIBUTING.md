# Contributing

`dwm_status` is a small Linux/X11 program with machine-specific edges. Keep
changes narrow, observable, and easy to revert.

## Before writing code

- Search the existing issues before opening another one.
- Open an issue before changing behavior, configuration, dependencies, output,
  or a public interface. Describe the machine and `dwm` setup the change serves.
- Send security reports through the private route in
  [`SECURITY.md`](SECURITY.md), not a public issue.
- Keep general usage questions and bug reports separate; see
  [`SUPPORT.md`](SUPPORT.md).

Maintainers control the project board. An issue is Done only after its change is
merged into `develop` and the exact merged commit passes CI.

## Branches and pull requests

1. Branch from the current `develop` branch.
2. Make one focused change per branch and commit.
3. Open the pull request against `develop`, never directly against `master`.
4. Link the issue and describe the user-visible effect, sharp edges, and checks
   run.
5. Update `README.md` in the same change when behavior, configuration, runtime
   dependencies, output order, supported environments, or side effects change.

`master` is owner-controlled. Promotion from `develop` happens only after the
installed program has been checked in a real X session.

## Local checks

Use Rust 1.85 or newer. Run these before opening a pull request:

```bash
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release
```

Run the relevant smoke tests when touching their paths:

```bash
./scripts/install-smoke.sh
./scripts/reinstall-smoke.sh
./scripts/xvfb-smoke.sh
./scripts/netlink-smoke.sh
```

The X11 and netlink smoke tests need the Linux utilities listed in the CI
workflow. CI also verifies the minimum Rust version and runs `cargo audit`.

Do not run `cargo fmt`. The repository deliberately preserves its local style
to keep diffs reviewable.

## Code and documentation rules

- Add a short `///` comment to every function and callback. The comment must add
  context instead of repeating the name.
- Prefix private helper functions with `_`, put them at the bottom of the impl or
  file, and extract one only when logic is duplicated.
- Name constructor-style helpers after their inputs. Use `from_*()` for parsers
  and `to_*()` for formatters.
- Write documentation for Linux users who already understand `dwm`, `xsetroot`,
  and hand-edited dotfiles.
- Document current behavior only. State vendor commands, root-window writes,
  process control, file writes, and other machine-specific assumptions plainly.

## License

By contributing, you agree that your contribution is licensed under the
[MIT License](LICENSE).
