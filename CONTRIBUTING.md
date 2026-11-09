# Contributing to Sunny

Thanks for wanting to help. Sunny is a small, focused project — the best contributions are the ones
that keep it small and focused.

Before you write code, read [`AGENTS.md`](AGENTS.md). It documents the architecture and the rules
that keep the project coherent. This file covers the process around it.

---

## Ways to contribute

- **Report a bug** — open an issue with steps to reproduce.
- **Suggest a feature** — open a discussion first if it is large.
- **Fix a bug / implement a feature** — see below.
- **Improve docs** — README, ROADMAP, or in-code comments that explain a non-obvious hack.
- **Translate** — not set up yet; open an issue if you want to help.

---

## Raising issues

- Search existing issues first; duplicates get closed.
- Use the issue template. A good bug report has: what you did, what you expected, what happened,
  your Windows version, and the Sunny version (Settings → About Sunny).
- For crashes, include the relevant part of the log or a screenshot.
- **Security issues**: do not open a public issue. Contact the maintainer privately
  (see the GitHub profile of [@aliplfrh](https://github.com/aliplfrh)).

---

## Contributing code

### 1. Discuss big changes first

Small fixes (typos, a bug, a small UI tweak) can go straight to a PR. Anything that touches
architecture — a new crate, a new window, a new dependency, a change to the settings format — should
be discussed in an issue first. Large PRs that were never discussed are likely to be rejected.

### 2. Keep the scope tight

One PR = one concern. Do not mix a bug fix with a refactor and a formatting pass. Reviewers cannot
review what they cannot isolate.

### 3. Respect the stack

Sunny is **Rust + Tauri v2 + vanilla HTML/CSS/JS**. Do not introduce a frontend framework, a bundler,
or a Node build step. Do not add a dependency unless it clearly earns its place — this app is
deliberately lightweight.

### 4. Test locally

Every PR must be built and run locally before submission:

```powershell
cargo fmt --all
cargo test -p sun-core
cargo build --release
target\release\sunny.exe
```

Remember that UI files are embedded at compile time — you must rebuild to see changes.

### 5. Formatting and lints

- `cargo fmt --all` is mandatory. CI will not accept unformatted Rust.
- `cargo clippy` should be clean for new code.
- The frontend has no formatter; match the existing style (2-space indent, double quotes, semicolons).

### 6. Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add window switcher
fix: keep settings window alive on close
docs: clarify the release process
chore: bump tauri to 2.12
```

Write the subject in the imperative mood, keep it under ~72 characters, and explain *why* in the body
when the change is not obvious. English only.

### 7. Pull requests

- Describe **what** changed and **why**, briefly. No novels.
- Link the issue it closes.
- Include a screenshot or short clip for UI changes.
- State how you tested it.
- Keep the description honest about AI assistance (see below).

---

## AI-generated code

AI-assisted contributions are welcome, and are held to the same standard as everything else.

- **You are responsible for the code you submit.** Read it, understand it, and test it. Do not paste
  output you have not verified.
- **Follow `AGENTS.md`.** It exists precisely so that an agent can be productive without breaking the
  architecture.
- **Say so.** If the contribution was mostly AI-generated, mention the model/tool in the PR
  description. This is good practice, not a penalty.
- **Lazy PRs are rejected.** Unformatted, untested, or architecture-breaking AI output will be closed
  without a long review.

---

## Code of conduct

Be decent. Assume good faith, critique the code and not the person, and keep discussions on topic.
Harassment or hostility is not tolerated.

---

## License

By contributing, you agree that your contribution is licensed under **GPL-3.0**, the same license as
the rest of the project. Sunny is a derivative work of Vicinae; see [`NOTICE`](NOTICE).
