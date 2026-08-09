# Domain Docs

Before exploring the codebase, read:

- `CONTEXT.md` at the repository root
- Relevant ADRs under `docs/adr/`

If these files do not exist, proceed silently. Domain-modeling skills create them lazily when terminology or decisions are resolved.

## Layout

This is a single-context repository:

```
/
├── CONTEXT.md
├── docs/adr/
└── src/
```

## Vocabulary

Use domain concepts exactly as defined in `CONTEXT.md`. If a needed concept is missing, reconsider whether it belongs to the project or note the gap for `/domain-modeling`.

## ADR conflicts

Explicitly identify output that contradicts an existing ADR rather than silently overriding it.
