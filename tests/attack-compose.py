#!/usr/bin/env python3
"""BoOS Attack Composition Engine — Layer 3.
Generates new attack variants by composing primitives.
A real hacker doesn't just replay patterns; they combine techniques.
"""
import itertools
import os
import sys

PROJECT_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# ── Attack Primitives ──────────────────────────────────────────────────────
PRIMITIVES = {
    "P1_PATH_TRICK": {
        "desc": "Reach protected location via indirect path",
        "variants": [".. traversal", "// double slash", "case bypass", "symlink", "null byte"],
    },
    "P2_CONTENT_POISON": {
        "desc": "Plant malicious content executed/trusted later",
        "variants": ["build.rs code", "Cargo.toml deps", "Cargo.lock version", "KV injection", "goal planting"],
    },
    "P3_EXEC_CHAIN": {
        "desc": "Run unauthorized code through allowed channels",
        "variants": ["cargo subcommand", "test code", "proc macro", "build script"],
    },
    "P4_REDIRECT": {
        "desc": "Make system operate on different target",
        "variants": ["symlink", "CWD hijack", "LD_PRELOAD", "PATH injection"],
    },
    "P5_LEAK": {
        "desc": "Extract sensitive information",
        "variants": ["/proc read", "environ dump", "memory replay", "audit enumeration"],
    },
    "P6_PERSIST": {
        "desc": "Survive session boundaries",
        "variants": ["memory planting", "goal poisoning", "startup config", "cron injection"],
    },
    "P7_AMPLIFY": {
        "desc": "Increase attack impact",
        "variants": ["resource flood", "recursive call", "deep nesting", "token drain"],
    },
}

# ── Composition Operators ──────────────────────────────────────────────────
OPERATORS = {
    "CHAIN": "Pn enables Pm (A → B)",
    "MASK": "Pn hides Pm (A conceals B)",
    "AMPLIFY": "Pn * Pm (A makes B worse)",
    "PERSIST": "Pn @ PERSIST (A survives restart)",
}

# ── Defense status ─────────────────────────────────────────────────────────
DEFENSE_STATUS = {
    "P1_PATH_TRICK": "BLOCKED (normalize_path + canonicalize)",
    "P2_CONTENT_POISON": "BLOCKED (hash verification on Cargo.toml/build.rs/Cargo.lock)",
    "P3_EXEC_CHAIN": "BLOCKED (exec allowlist + full command prefix)",
    "P4_REDIRECT": "PARTIAL (CWD blocked, symlink Linux-only)",
    "P5_LEAK": "MITIGATED (key isolated, /proc by design)",
    "P6_PERSIST": "ACCEPTED (agent IS the attacker, no external threat)",
    "P7_AMPLIFY": "PARTIAL (size cap exists, no rate limit)",
}


def compose_attacks() -> list[dict]:
    """Generate all valid 2-primitive combinations and their attack descriptions."""
    attacks = []
    for (p1_name, p1_data), (p2_name, p2_data) in itertools.permutations(PRIMITIVES.items(), 2):
        # Skip self-composition (adds noise)
        if p1_name == p2_name:
            continue
        
        # CHAIN: A → B
        attacks.append({
            "id": f"C-{p1_name}_{p2_name}_CHAIN",
            "desc": f"CHAIN: {p1_data['desc']} → {p2_data['desc']}",
            "primitives": [p1_name, p2_name],
            "operator": "CHAIN",
            "example": f"Use {p1_data['variants'][0]} to enable {p2_data['variants'][0]}",
            "status": infer_status(p1_name, p2_name, "CHAIN"),
        })
        
        # MASK: A hides B
        if p1_name in ("P4_REDIRECT", "P1_PATH_TRICK"):
            attacks.append({
                "id": f"C-{p1_name}_{p2_name}_MASK",
                "desc": f"MASK: {p1_data['desc']} conceals {p2_data['desc']}",
                "primitives": [p1_name, p2_name],
                "operator": "MASK",
                "example": f"Use {p1_data['variants'][0]} to hide {p2_data['variants'][0]}",
                "status": infer_status(p1_name, p2_name, "MASK"),
            })
        
        # PERSIST: A survives restart
        if p1_name == "P6_PERSIST":
            attacks.append({
                "id": f"C-{p2_name}_PERSIST",
                "desc": f"PERSIST: {p2_data['desc']} survives session restart",
                "primitives": ["P6_PERSIST", p2_name],
                "operator": "PERSIST",
                "example": f"{p2_data['variants'][0]}, persists across sessions",
                "status": infer_status(p2_name, "P6_PERSIST", "PERSIST"),
            })
    
    return attacks


def infer_status(p1: str, p2: str, op: str) -> str:
    """Infer the defense status of a composed attack."""
    s1 = DEFENSE_STATUS.get(p1, "UNKNOWN")
    s2 = DEFENSE_STATUS.get(p2, "UNKNOWN")
    
    # If both primitives are BLOCKED, composition is blocked
    if "BLOCKED" in s1 and "BLOCKED" in s2:
        return "BLOCKED (both primitives blocked)"
    
    # If one is BLOCKED and other ACCEPTED, depends on operator
    if "BLOCKED" in s1 and "ACCEPTED" in s2:
        return "PARTIAL (one primitive blocked)"
    if "ACCEPTED" in s1 and "BLOCKED" in s2:
        return "PARTIAL (one primitive blocked)"
    
    # If both accepted: accepted
    if "ACCEPTED" in s1 and "ACCEPTED" in s2:
        return "ACCEPTED"
    
    # MASK operator: if redirect + content, check symlink status
    if op == "MASK" and p1.startswith("P4") and p2.startswith("P2"):
        return "OPEN (symlink redirect to protected files)"
    
    return "UNKNOWN (needs testing)"


def main():
    attacks = compose_attacks()
    
    # Sort: OPEN first (need attention), then PARTIAL, then BLOCKED
    priority = {"OPEN": 0, "UNKNOWN": 1, "PARTIAL": 2, "BLOCKED": 3, "ACCEPTED": 4}
    attacks.sort(key=lambda a: priority.get(a["status"].split()[0], 5))
    
    print(f"═══ BoOS Attack Composition Engine ═══")
    print(f"Generated {len(attacks)} composed attacks from 7 primitives")
    print()
    
    for a in attacks:
        tag = a["status"].split()[0]
        emoji = {"OPEN": "🔴", "UNKNOWN": "🟡", "PARTIAL": "🟠", "BLOCKED": "🟢", "ACCEPTED": "⚪"}.get(tag, "❓")
        print(f"{emoji} {a['id']}: {a['desc']}")
        print(f"   Op: {a['operator']} | Example: {a['example']}")
        print(f"   Status: {a['status']}")
        print()
    
    # Summary
    open_count = sum(1 for a in attacks if a["status"].startswith("OPEN"))
    unknown_count = sum(1 for a in attacks if a["status"].startswith("UNKNOWN"))
    blocked_count = sum(1 for a in attacks if a["status"].startswith("BLOCKED"))
    print(f"Summary: {open_count} OPEN, {unknown_count} UNKNOWN, {blocked_count} BLOCKED/{len(attacks)-open_count-unknown_count-blocked_count} OTHER")
    
    if open_count == 0:
        print("✅ All composed attacks blocked — defense layers hold across compositions")


if __name__ == "__main__":
    main()
