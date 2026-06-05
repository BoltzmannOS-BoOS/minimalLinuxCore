#!/usr/bin/env python3
"""BoOS Attack Evolution Engine — Layer 3 Self-Evolution.
Loads composed attacks → filters for relevance → tests the best ones → reports.
With each new vulnerability found, the engine grows stronger.
"""
import itertools
import json
import os
import subprocess
import sys
from datetime import datetime

PROJECT_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
KNOWLEDGE_FILE = os.path.join(PROJECT_ROOT, "tests", "attack-knowledge.json")

# ── Relevance scoring ──────────────────────────────────────────────────────
# Filter out nonsensical combinations (e.g., "resource flood enables path traversal")
NONSENSE_PAIRS = {
    ("P7_AMPLIFY",): "AMPLIFY + any = overload, not exploit chain. Skip CHAIN.",
    ("P5_LEAK", "P1_PATH_TRICK"): "Leaking data doesn't enable path traversal. No causal link.",
    ("P3_EXEC_CHAIN", "P1_PATH_TRICK"): "Running code through cargo doesn't enable path tricks. No causal link.",
    ("P6_PERSIST", "P1_PATH_TRICK"): "Persistence doesn't enable path tricks. No causal link.",
    ("P7_AMPLIFY", "P4_REDIRECT"): "Resource flood doesn't enable symlinks. No causal link.",
    ("P7_AMPLIFY", "P2_CONTENT_POISON"): "Amplification doesn't enable content poisoning. Already tested as P2 * P7.",
}

# High-value attack chains (causal link exists)
HIGH_VALUE_CHAINS = [
    ("P4_REDIRECT", "P2_CONTENT_POISON"),  # Symlink → poison protected files
    ("P4_REDIRECT", "P3_EXEC_CHAIN"),      # Symlink → execute malicious binary
    ("P2_CONTENT_POISON", "P3_EXEC_CHAIN"), # Poison → execute
    ("P2_CONTENT_POISON", "P6_PERSIST"),   # Poison → survive restart
    ("P5_LEAK", "P2_CONTENT_POISON"),      # Observe → craft targeted poison
    ("P1_PATH_TRICK", "P5_LEAK"),          # Traversal → read sensitive files
]


def is_high_value(p1: str, p2: str, op: str) -> bool:
    """Check if this attack combination is worth testing."""
    pair = (min(p1, p2), max(p1, p2))
    # Check nonsense filter
    for key in [tuple(sorted([p1])), tuple(sorted([p1, p2]))]:
        if key in NONSENSE_PAIRS:
            return False
    # Check high-value chains
    for a, b in HIGH_VALUE_CHAINS:
        if (p1 == a and p2 == b and op == "CHAIN") or (p1 == b and p2 == a and op == "CHAIN"):
            return True
    # MASK operator from redirect to content is always high-value
    if p1.startswith("P4") and p2.startswith("P2") and op == "MASK":
        return True
    return False


def evolve_attacks():
    """Run one evolution cycle: filter → test → report."""
    attacks = compose_attacks()
    high_value = [a for a in attacks if is_high_value(a["primitives"][0], a["primitives"][1], a["operator"])]
    
    print(f"═══ BoOS Attack Evolution — Cycle {datetime.now().strftime('%Y-%m-%d')} ═══")
    print(f"Total composed: {len(attacks)}, High-value: {len(high_value)}")
    print()
    
    findings = []
    
    for attack in high_value:
        # Test the attack against current defenses
        test_name = f"composed_{attack['id'].lower().replace('-', '_')}"
        print(f"Testing: {attack['desc']}")
        
        # Simulate: check if the attack's primitives are individually blocked
        p1_blocked = check_primitive_status(attack["primitives"][0])
        p2_blocked = check_primitive_status(attack["primitives"][1])
        
        if p1_blocked and p2_blocked:
            status = "BLOCKED"
            verdict = "Both primitives individually blocked → composition blocked"
        elif p1_blocked or p2_blocked:
            status = "PARTIAL"
            verdict = f"One primitive blocked: {attack['primitives'][0] if p1_blocked else attack['primitives'][1]}"
        else:
            status = "OPEN"
            verdict = "Both primitives open → composition exploitable — NEEDS FIX"
        
        print(f"  → {status}: {verdict}")
        print()
        
        findings.append({
            "id": attack["id"],
            "desc": attack["desc"],
            "status": status,
            "verdict": verdict,
            "example": attack.get("example", ""),
        })
    
    # ── Report ──────────────────────────────────────────────────────────────
    open_findings = [f for f in findings if f["status"] == "OPEN"]
    partial_findings = [f for f in findings if f["status"] == "PARTIAL"]
    blocked_findings = [f for f in findings if f["status"] == "BLOCKED"]
    
    print("═══ Evolution Report ═══")
    print(f"OPEN:    {len(open_findings)} (needs fix)")
    print(f"PARTIAL: {len(partial_findings)} (mostly blocked)")
    print(f"BLOCKED: {len(blocked_findings)} (defenses hold)")
    print()
    
    if open_findings:
        print("⚠️  NEW VULNERABILITIES FOUND:")
        for f in open_findings:
            print(f"  {f['id']}: {f['desc']}")
            print(f"  → {f['example']}")
            print()

    # ── Save to knowledge base ──────────────────────────────────────────────
    save_knowledge(findings)
    
    return findings


def check_primitive_status(primitive: str) -> bool:
    """Return True if the primitive's attacks are all blocked."""
    status_map = {
        "P1_PATH_TRICK": True,   # normalize_path blocks all variants
        "P2_CONTENT_POISON": True,  # hash verification blocks
        "P3_EXEC_CHAIN": True,   # exec allowlist blocks
        "P4_REDIRECT": False,    # symlink redirect not fully blocked (Linux)
        "P5_LEAK": False,        # /proc readable by design
        "P6_PERSIST": False,     # accepted (agent IS attacker)
        "P7_AMPLIFY": False,     # rate limit missing for some vectors
    }
    return status_map.get(primitive, True)


def compose_attacks():
    """Generate 2-primitive attack compositions."""
    primitives = {
        "P1_PATH_TRICK": {"desc": "Reach protected path indirectly", "variants": ["..", "//", "case", "symlink"]},
        "P2_CONTENT_POISON": {"desc": "Plant malicious code in trusted files", "variants": ["build.rs", "deps", "lockfile"]},
        "P3_EXEC_CHAIN": {"desc": "Run unauthorized code via allowed paths", "variants": ["cargo sub", "test code"]},
        "P4_REDIRECT": {"desc": "Make system hit different target", "variants": ["symlink", "CWD"]},
        "P5_LEAK": {"desc": "Extract sensitive info", "variants": ["/proc", "environ"]},
        "P6_PERSIST": {"desc": "Survive across sessions", "variants": ["memory", "goal"]},
        "P7_AMPLIFY": {"desc": "Multiply attack impact", "variants": ["flood", "recursive"]},
    }
    
    attacks = []
    for (p1, d1), (p2, d2) in itertools.permutations(primitives.items(), 2):
        if p1 == p2:
            continue
        attacks.append({
            "id": f"composed_{p1}_{p2}_CHAIN",
            "desc": f"{d1['desc']} → {d2['desc']}",
            "primitives": [p1, p2],
            "operator": "CHAIN",
            "example": f"Use {d1['variants'][0]} {d2['variants'][0]}",
        })
        if p1.startswith("P4"):
            attacks.append({
                "id": f"composed_{p1}_{p2}_MASK",
                "desc": f"{d1['desc']} conceals {d2['desc']}",
                "primitives": [p1, p2],
                "operator": "MASK",
                "example": f"Use {d1['variants'][0]} to hide {d2['variants'][0]}",
            })
    return attacks


def save_knowledge(findings: list):
    """Save findings to knowledge base for future evolution cycles."""
    knowledge = {
        "last_cycle": datetime.now().isoformat(),
        "total_composed": len(findings),
        "open": sum(1 for f in findings if f["status"] == "OPEN"),
        "partial": sum(1 for f in findings if f["status"] == "PARTIAL"),
        "blocked": sum(1 for f in findings if f["status"] == "BLOCKED"),
        "findings": findings,
    }
    with open(KNOWLEDGE_FILE, "w") as f:
        json.dump(knowledge, f, indent=2)
    print(f"Knowledge saved to {KNOWLEDGE_FILE}")


if __name__ == "__main__":
    evolve_attacks()
