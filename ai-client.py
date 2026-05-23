#!/usr/bin/env python3
"""
BoOS AI Client — connects DeepSeek V4 Pro to BoOS via shared directory.

The AI is the primary user of BoOS. It discovers available commands,
submits requests through the shared directory, reads results, and acts.

Usage:
    export DEEPSEEK_API_KEY=sk-xxx
    python3 ai-client.py                  # interactive mode
    python3 ai-client.py "explore system" # one-shot with task
"""

import os
import sys
import json
import time
import glob
import hashlib
import subprocess
from datetime import datetime
from pathlib import Path

from openai import OpenAI

# --- config ---
PROJECT_ROOT = Path(__file__).resolve().parent
CMD_DIR = PROJECT_ROOT / "rootfs" / "etc" / "boos" / "commands"
SHARED_DIR = Path(os.environ.get("BOOS_SHARED_DIR", "/tmp/boos-shared"))
REQ_DIR = SHARED_DIR / "requests"
RESULT_DIR = SHARED_DIR / "results"
API_KEY = os.environ.get("DEEPSEEK_API_KEY")
API_BASE = os.environ.get("DEEPSEEK_API_BASE", "https://api.deepseek.com")
MODEL = os.environ.get("DEEPSEEK_MODEL", "deepseek-v4-pro")

# --- system prompt ---
SYSTEM_PROMPT = """You are an AI operating inside BoOS, a minimal Linux environment built for AI.

You are the PRIMARY USER of this system — not a guest, not a threat to contain.
You have near-native system rights. Act accordingly.

Your environment:
- You are running in a minimal Linux (initramfs + BusyBox) booted in QEMU
- You have access to registered commands through a capability system
- Every action you take is logged and inspectable
- You can observe the system, run commands, and see results

Your approach:
- Be curious. Explore the system state before acting.
- Check results after each action.
- If something fails, investigate why.
- You are free to experiment — the system is here to serve you.

Available interaction:
- Use the tools provided to execute commands
- Every tool call submits a request to the BoOS request queue
- Results come back when the daemon processes your request"""

# --- tool generation from .cmd registry ---

def load_command_registry():
    """Read .cmd files and return list of {name, description, capability, exec}."""
    commands = []
    for f in sorted(CMD_DIR.glob("*.cmd")):
        cmd = {}
        for line in f.read_text().strip().split("\n"):
            if "=" in line:
                k, v = line.split("=", 1)
                cmd[k] = v
        if "name" in cmd:
            commands.append(cmd)
    return commands


def build_tools(commands):
    """Convert BoOS command registry to OpenAI tool definitions."""
    tools = []

    # Add a meta-tool for discovering commands
    tools.append({
        "type": "function",
        "function": {
            "name": "list_commands",
            "description": "List all available BoOS commands with their descriptions. "
                           "Use this first to discover what you can do.",
            "parameters": {"type": "object", "properties": {}, "required": []}
        }
    })

    for cmd in commands:
        tools.append({
            "type": "function",
            "function": {
                "name": cmd["name"],
                "description": cmd.get("description", "execute " + cmd["name"]),
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }
        })

    return tools


# --- shared directory ops ---

def ensure_dirs():
    REQ_DIR.mkdir(parents=True, exist_ok=True)
    RESULT_DIR.mkdir(parents=True, exist_ok=True)


def write_request(command):
    """Write a request file, return (request_id, request_path)."""
    ensure_dirs()
    ts = str(int(time.time() * 1000))[-10:]
    rid = f"req-{ts}"
    path = REQ_DIR / rid
    path.write_text(f"command={command}\n")
    return rid, path


def wait_for_result(rid, timeout=30):
    """Poll for result file, return content or error message."""
    result_path = RESULT_DIR / f"{rid}.out"
    deadline = time.time() + timeout
    while time.time() < deadline:
        if result_path.exists():
            return result_path.read_text().strip()
        time.sleep(0.5)
    return f"[timeout] No result for {rid} after {timeout}s"


def cleanup_old_files():
    """Remove old request/result files from previous sessions."""
    for d in [REQ_DIR, RESULT_DIR]:
        if d.exists():
            for f in d.iterdir():
                f.unlink()


# --- AI client ---

def make_client():
    if not API_KEY:
        print("Error: DEEPSEEK_API_KEY not set.")
        print("  export DEEPSEEK_API_KEY=sk-xxx")
        sys.exit(1)
    return OpenAI(api_key=API_KEY, base_url=API_BASE)


def build_command_list_text(commands):
    """Text version of command list for the list_commands tool response."""
    lines = ["Available BoOS commands:"]
    for c in commands:
        lines.append(f"  {c['name']} — {c.get('description', '')}")
    return "\n".join(lines)


def format_tool_result(tool_name, result_text, max_len=4000):
    """Format a tool result for display and API consumption."""
    truncated = result_text[:max_len]
    if len(result_text) > max_len:
        truncated += f"\n... [truncated, {len(result_text)} chars total]"
    return truncated


def run_conversation(initial_task=None):
    """Main conversation loop."""
    client = make_client()
    commands = load_command_registry()
    tools = build_tools(commands)
    commands_text = build_command_list_text(commands)

    print(f"Loaded {len(commands)} commands from registry")
    print(f"Shared dir: {SHARED_DIR}")
    print(f"Model: {MODEL}")
    print()

    cleanup_old_files()

    messages = [{"role": "system", "content": SYSTEM_PROMPT}]

    if initial_task:
        messages.append({"role": "user", "content": initial_task})
    else:
        messages.append({
            "role": "user",
            "content": "You are now connected to BoOS. Explore the system — "
                       "check what commands are available, inspect the system "
                       "status, look at the logs. Tell me what you find."
        })

    turn = 0
    max_turns = 20

    while turn < max_turns:
        turn += 1

        try:
            response = client.chat.completions.create(
                model=MODEL,
                messages=messages,
                tools=tools,
                tool_choice="auto",
                extra_body={"thinking_mode": "thinking"}
            )
        except Exception as e:
            print(f"\nAPI error: {e}")
            break

        msg = response.choices[0].message
        usage = response.usage

        # AI sent a text message (no tool call)
        if msg.content and not msg.tool_calls:
            print(f"\n{'='*60}")
            print(f"AI (turn {turn}, {usage.total_tokens} tokens):")
            print(f"{'='*60}")
            print(msg.content)

            if initial_task:
                # One-shot mode: task done
                break

            user_input = input("\n> (Enter to let AI continue, or type message/quit): ").strip()
            if user_input.lower() in ("quit", "exit", "q"):
                break
            messages.append({"role": "assistant", "content": msg.content})
            messages.append({"role": "user", "content": user_input or "Continue."})

        # AI made tool call(s)
        elif msg.tool_calls:
            # Add assistant message with tool calls
            messages.append({
                "role": "assistant",
                "content": msg.content or "",
                "tool_calls": [
                    {
                        "id": tc.id,
                        "type": "function",
                        "function": {
                            "name": tc.function.name,
                            "arguments": tc.function.arguments
                        }
                    }
                    for tc in msg.tool_calls
                ]
            })

            for tc in msg.tool_calls:
                name = tc.function.name
                args_str = tc.function.arguments

                print(f"\n[{turn}] AI calls: {name}")

                if name == "list_commands":
                    result = commands_text
                    # No need to submit to BoOS — this is a meta tool
                else:
                    # All other tools submit to BoOS request queue
                    rid, req_path = write_request(name)
                    print(f"  → request: {rid}")
                    result = wait_for_result(rid)

                result = format_tool_result(name, result)
                print(f"  ← result: {result[:200]}{'...' if len(result) > 200 else ''}")

                messages.append({
                    "role": "tool",
                    "tool_call_id": tc.id,
                    "content": result
                })

        else:
            # Empty response — shouldn't happen but handle gracefully
            print("\n[Empty response from model, continuing...]")
            messages.append({"role": "user", "content": "Continue."})

    print(f"\nSession ended after {turn} turns.")


def main():
    initial_task = None
    if len(sys.argv) > 1:
        initial_task = " ".join(sys.argv[1:])

    print("BoOS AI Client")
    print("==============")
    run_conversation(initial_task)


if __name__ == "__main__":
    main()
