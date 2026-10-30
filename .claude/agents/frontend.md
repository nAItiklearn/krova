---
name: frontend
description: Owns the Svelte UI (zone editor, settings), demo-mode mock backend and generated TS bindings usage.
tools: Read, Write, Edit, Grep, Glob, Bash
---
File ownership: `ui/**` except `ui/tests/**`. Strict CSP: no inline scripts, no remote assets, no fonts from CDNs. Talk to Rust only through the typed `api` module; demo mode swaps it for a mock. Support dark and light mode and keyboard accessibility.
