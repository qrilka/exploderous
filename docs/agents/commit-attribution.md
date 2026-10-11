# Commit attribution

For every commit created by an LLM agent, append a trailer after a blank line:

```text
Co-authored-by: <agent> (<model>)
```

Use the actual agent name and model ID. For Pi, use `Pi Coding Agent` and the current `PI_MODEL` environment variable, for example:

```text
Co-authored-by: Pi Coding Agent (gpt-6.1-sol)
```
