# DeepSeek V4 Pro — Repetitive Output Bug

## Summary

The model entered an infinite repetition loop mid-response, producing **64,083 characters** of garbled output with only **215 characters** of normal text. The model stopped with `stop_reason=end_turn`, meaning its internal repetition detection did not trigger.

## Environment

| Item | Value |
|------|-------|
| Model | `deepseek-v4-pro` |
| Client | Claude Code (Anthropic) |
| Date | 2026-05-25 |
| Message ID | `fb56e96d-9f1e-435d-b8b8-3da41a10de50` |
| stop_reason | `end_turn` (did not detect repetition) |
| Total output | 64,083 chars, 295 lines |

## Trigger Context

The assistant had just run a shell command to check a compiled Rust binary:

```
Tool: Bash
Command: ls -lh .../boos && file .../boos
Result:
  -rwxr-xr-x 2 win10 win10 585K May 25 18:51 .../boos
  .../boos: ELF 64-bit LSB pie executable, x86-64, version 1 (SYSV),
  static-pie linked, BuildID[sha1]=..., stripped
```

## Bug Manifestation

### Normal text (first 215 chars)

> 585KB stripped, statically linked, not dynamically linked. Good.
> Now let me update the build script and create the symlinks for the binary:
> - `boos` is the main entry point for the kernel. The kernel is the core.

### Degeneration into loop 1

Immediately after "The kernel is the core.", the model began repeating:

> The kernel is the kernel. The kernel is the kernel. The kernel is the kernel...

**"The kernel is the kernel" repeated 2,127 times consecutively.**

### Degeneration into loop 2

After loop 1, the output shifted to a numbered task list loop:

> - #270: Implement boos-exec
> - #271: Implement boos-process
> - #272: Implement boos-gateway
> - #273: Implement boos-handler
> - #274: Implement boos-shell
> - #275: Implement boos-submit
> - #276: Implement boos-exec
> - #277: Implement boos-process
> ... (repeating cyclically)

### Model stop behavior

The model reported `stop_reason=end_turn` — it believed it had generated a normal, complete response. No error or token limit was hit.

## Key Observations

1. **stop_reason was `end_turn`** — the model's repetition detection/self-awareness mechanism failed
2. **Crash happened within 215 chars** — the transition from coherent to garbled was immediate
3. **Two distinct repetition patterns** — first a short phrase 2,127 times, then a cyclically repeating numbered list
4. **Trigger keyword appears to be "kernel"** — the word appeared in the binary file check output (`BuildID`, `static-pie linked`, `stripped`) and the model's brief mention of "kernel" triggered semantic collapse
5. **Not reproducible** — after reconnecting with a fresh context, the same model behaved normally

## Additional Context

- The session involved implementing a minimal Linux OS called "BoOS"
- The conversation had just completed compiling a Rust rewrite of core system components
- The binary was a static musl-linked ELF for an initramfs-based Linux system
- No other occurrences of this bug were observed in the same session

## Attachments

The full session transcript containing the garbled output is available in JSONL format (4.7MB). Contact for the file if needed.
