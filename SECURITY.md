# Security Policy

## Supported code

Only the current `master` branch is supported. The project has no tagged
releases and does not provide security fixes for older commits or development
branches.

## Report a vulnerability

Use [GitHub private vulnerability reporting](https://github.com/euvoor/dwm-status/security/advisories/new).
Do not open a public issue, discussion, or pull request for a suspected
vulnerability.

Include:

- the affected commit and platform;
- the security impact and required attacker access;
- minimal reproduction steps or a proof of concept;
- any known workaround.

Configuration files are trusted local input, and `dwm_status` runs with the
permissions and X11 access of the invoking user. Reports should distinguish a
security boundary failure from behavior already documented in `README.md`, such
as root-window property writes, `nvidia-smi` execution, and process replacement
by `make reinstall`.

No response or disclosure deadline is promised. Please allow time to reproduce
and fix the problem before publishing details.
