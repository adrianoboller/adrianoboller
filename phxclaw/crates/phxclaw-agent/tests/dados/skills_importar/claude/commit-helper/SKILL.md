---
name: commit-helper
description: "Writes commit messages from the staged diff: \"why\" before \"what\"."
allowed-tools: Read, Grep, Bash(git:*)
---

# Commit helper

1. Run `Bash` with `git diff --cached`.
2. Use `Read` on the changed files when the diff is not enough.
3. Write the message. Read the guidelines first.
