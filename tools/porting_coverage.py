#!/usr/bin/env python3
"""Measure ZMK-to-RMK porting coverage from a manifest.

The manifest is the migration contract. It is intentionally explicit: each
keymap row, behavior value, combo, and high-risk thumb-layer scenario has an
expected RMK result derived from the upstream ZMK implementation plus documented
RMK-specific deltas.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any


TRANSPARENT = "_"
NO_KEY = "No"
LAYER_NAMES = ["DEFAULT_LAYER", "SECONDARY_LAYER", "TERTIARY_LAYER", "SYSTEM_LAYER"]

ZMK_KEY_TO_RMK = {
    "A": "A",
    "B": "B",
    "C": "C",
    "D": "D",
    "E": "E",
    "F": "F",
    "G": "G",
    "H": "H",
    "I": "I",
    "J": "J",
    "K": "K",
    "L": "L",
    "M": "M",
    "N": "N",
    "O": "O",
    "P": "P",
    "Q": "Q",
    "R": "R",
    "S": "S",
    "T": "T",
    "U": "U",
    "V": "V",
    "W": "W",
    "X": "X",
    "Y": "Y",
    "Z": "Z",
    "NUMBER_1": "Kc1",
    "NUMBER_2": "Kc2",
    "NUMBER_3": "Kc3",
    "NUMBER_4": "Kc4",
    "NUMBER_5": "Kc5",
    "NUMBER_6": "Kc6",
    "NUMBER_7": "Kc7",
    "NUMBER_8": "Kc8",
    "NUMBER_9": "Kc9",
    "NUMBER_0": "Kc0",
    "N1": "Kc1",
    "N2": "Kc2",
    "N3": "Kc3",
    "N4": "Kc4",
    "N5": "Kc5",
    "N6": "Kc6",
    "N7": "Kc7",
    "N8": "Kc8",
    "N9": "Kc9",
    "N0": "Kc0",
    "KP_NUMLOCK": "NumLock",
    "KP_NUMBER_7": "Kp7",
    "KP_NUMBER_8": "Kp8",
    "KP_NUMBER_9": "Kp9",
    "KP_NUMBER_4": "Kp4",
    "KP_NUMBER_5": "Kp5",
    "KP_NUMBER_6": "Kp6",
    "KP_NUMBER_1": "Kp1",
    "KP_NUMBER_2": "Kp2",
    "KP_NUMBER_3": "Kp3",
    "KP_NUMBER_0": "Kp0",
    "KP_PLUS": "KpPlus",
    "KP_MINUS": "KpMinus",
    "KP_ASTERISK": "KpAsterisk",
    "KP_DOT": "KpDot",
    "KP_DIVIDE": "KpSlash",
    "F1": "F1",
    "F2": "F2",
    "F3": "F3",
    "F4": "F4",
    "F5": "F5",
    "F6": "F6",
    "F7": "F7",
    "F8": "F8",
    "F9": "F9",
    "F10": "F10",
    "F11": "F11",
    "F12": "F12",
    "F13": "F13",
    "F14": "F14",
    "F15": "F15",
    "LEFT_SHIFT": "LShift",
    "LEFT_ALT": "LAlt",
    "LCTRL": "LCtrl",
    "LWIN": "LGui",
    "LANGUAGE_1": "Language1",
    "LANGUAGE_2": "Language2",
    "BACKSLASH": "Backslash",
    "BACKSPACE": "Backspace",
    "ENTER": "Enter",
    "SPACE": "Space",
    "EQUAL": "Equal",
    "MINUS": "Minus",
    "COMMA": "Comma",
    "PERIOD": "Dot",
    "SLASH": "Slash",
    "DEL": "Delete",
    "PAGE_UP": "PageUp",
    "PAGE_DOWN": "PageDown",
    "UP_ARROW": "Up",
    "DOWN_ARROW": "Down",
    "UP": "Up",
    "DOWN": "Down",
    "LEFT": "Left",
    "RIGHT": "Right",
    "HOME": "Home",
    "END": "End",
    "PRINTSCREEN": "PrintScreen",
    "SCROLLLOCK": "ScrollLock",
    "PAUSE_BREAK": "Pause",
    "EXCLAMATION": "SHIFTED(Kc1)",
    "AT_SIGN": "SHIFTED(Kc2)",
    "HASH": "SHIFTED(Kc3)",
    "DOLLAR": "SHIFTED(Kc4)",
    "PERCENT": "SHIFTED(Kc5)",
    "CARET": "SHIFTED(Kc6)",
    "AMPERSAND": "SHIFTED(Kc7)",
    "ASTERISK": "SHIFTED(Kc8)",
    "LEFT_PARENTHESIS": "SHIFTED(Kc9)",
    "RIGHT_PARENTHESIS": "SHIFTED(Kc0)",
    "GRAVE": "Grave",
    "SQT": "Quote",
    "SEMI": "Semicolon",
    "LEFT_BRACKET": "LeftBracket",
    "RIGHT_BRACKET": "RightBracket",
    "ESCAPE": "Escape",
    "TAB": "Tab",
}

ZMK_MOUSE_TO_RMK = {
    "LCLK": "MouseBtn1",
    "RCLK": "MouseBtn2",
    "MCLK": "MouseBtn3",
    "MB4": "MouseBtn4",
    "MB5": "MouseBtn5",
}

ZMK_DYNAMIC_SCALE_TO_RMK = {
    ("ZDS_XY", "ZDS_INC"): "User9",
    ("ZDS_XY", "ZDS_DEC"): "User10",
    ("ZDS_SC", "ZDS_INC"): "User11",
    ("ZDS_SC", "ZDS_DEC"): "User12",
    ("ZDS_ALL", "ZDS_RST"): "User13",
}


@dataclass
class Result:
    id: str
    kind: str
    passed: int
    total: int
    message: str

    @property
    def ok(self) -> bool:
        return self.passed == self.total


def load_toml(path: Path) -> dict[str, Any]:
    with path.open("rb") as f:
        return tomllib.load(f)


def path_get(root: dict[str, Any], dotted: str) -> Any:
    value: Any = root
    for part in dotted.split("."):
        if not isinstance(value, dict) or part not in value:
            raise KeyError(dotted)
        value = value[part]
    return value


def keymap(config: dict[str, Any]) -> list[list[list[str]]]:
    return config["layout"]["keymap"]


def combo_set(config: dict[str, Any]) -> set[tuple[tuple[str, ...], str, int]]:
    combos = config["behavior"]["combo"]["combos"]
    return {
        (tuple(combo["actions"]), combo["output"], int(combo["layer"]))
        for combo in combos
    }


def tap_action(action: str) -> str:
    if action.startswith("LT("):
        inside = action.removeprefix("LT(").removesuffix(")")
        pieces = [piece.strip() for piece in inside.split(",")]
        if len(pieces) >= 2:
            return pieces[1]
    return action


def active_layers_from_holds(config: dict[str, Any], holds: list[dict[str, Any]]) -> list[int]:
    active = {int(hold["activates_layer"]) for hold in holds}
    tri = config.get("behavior", {}).get("tri_layer", {})
    lower = tri.get("lower")
    upper = tri.get("upper")
    adjust = tri.get("adjust")
    if lower in active and upper in active and adjust is not None:
        active.add(int(adjust))
    return sorted(active)


def resolve_key(config: dict[str, Any], row: int, col: int, active_layers: list[int]) -> str:
    km = keymap(config)
    for layer in sorted(active_layers, reverse=True):
        action = km[layer][row][col]
        if action != TRANSPARENT:
            return tap_action(action)
    return tap_action(km[0][row][col])


def check_layout(manifest: dict[str, Any], config: dict[str, Any]) -> list[Result]:
    results: list[Result] = []
    expected = manifest["layout"]
    actual = config["layout"]
    for field in ("rows", "cols", "layers"):
        ok = actual.get(field) == expected.get(field)
        results.append(
            Result(
                id=f"layout.{field}",
                kind="layout",
                passed=1 if ok else 0,
                total=1,
                message=f"expected {expected.get(field)!r}, got {actual.get(field)!r}",
            )
        )
    return results


def check_keymap_rows(manifest: dict[str, Any], config: dict[str, Any]) -> list[Result]:
    results: list[Result] = []
    km = keymap(config)
    for row_check in manifest.get("keymap_rows", []):
        layer = int(row_check["layer"])
        row = int(row_check["row"])
        expected = list(row_check["expected"])
        actual = km[layer][row]
        passed = 0
        mismatches: list[str] = []
        for col, (want, got) in enumerate(zip(expected, actual, strict=True)):
            if want == got:
                passed += 1
            else:
                mismatches.append(f"c{col}: expected {want!r}, got {got!r}")
        message = "ok" if not mismatches else "; ".join(mismatches)
        results.append(Result(row_check["id"], "keymap_cell", passed, len(expected), message))
    return results


def check_behavior_values(manifest: dict[str, Any], config: dict[str, Any]) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("behavior_values", []):
        expected = check["expected"]
        try:
            actual = path_get(config, check["path"])
        except KeyError:
            actual = None
        ok = actual == expected
        results.append(
            Result(
                check["id"],
                "behavior",
                1 if ok else 0,
                1,
                f"expected {expected!r}, got {actual!r}",
            )
        )
    return results


def check_combos(manifest: dict[str, Any], config: dict[str, Any]) -> list[Result]:
    results: list[Result] = []
    actual = combo_set(config)
    for check in manifest.get("combos", []):
        expected = (tuple(check["actions"]), check["output"], int(check["layer"]))
        ok = expected in actual
        results.append(
            Result(
                check["id"],
                "combo",
                1 if ok else 0,
                1,
                "ok" if ok else f"missing {expected!r}",
            )
        )
    return results


def check_scenarios(manifest: dict[str, Any], config: dict[str, Any]) -> list[Result]:
    results: list[Result] = []
    km = keymap(config)
    for scenario in manifest.get("scenarios", []):
        holds = list(scenario.get("holds", []))
        if "hold" in scenario:
            holds.append(scenario["hold"])

        passed = 0
        total = 1 + len(holds)
        messages: list[str] = []
        for hold in holds:
            action = km[0][int(hold["row"])][int(hold["col"])]
            expected_action = hold["expected_action"]
            if action == expected_action:
                passed += 1
            else:
                messages.append(f"hold action expected {expected_action!r}, got {action!r}")

        active_layers = active_layers_from_holds(config, holds)
        tap = scenario["tap"]
        output = resolve_key(config, int(tap["row"]), int(tap["col"]), active_layers)
        expected_output = scenario["expected_output"]
        if output == expected_output:
            passed += 1
        else:
            messages.append(f"output expected {expected_output!r}, got {output!r}")

        results.append(
            Result(
                scenario["id"],
                "scenario",
                passed,
                total,
                "ok" if not messages else "; ".join(messages),
            )
        )
    return results


def extract_block(text: str, name: str) -> str:
    match = re.search(
        rf"(?<![A-Za-z0-9_]){re.escape(name)}(?![A-Za-z0-9_])(?:\s*:[^{{]+)?\s*\{{",
        text,
    )
    if not match:
        raise ValueError(f"block {name!r} not found")

    start = match.end() - 1
    depth = 0
    for index in range(start, len(text)):
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                return text[start + 1 : index]
    raise ValueError(f"block {name!r} is not closed")


def extract_angle_property(block: str, name: str) -> str:
    match = re.search(rf"\b{re.escape(name)}\s*=\s*<(?P<body>.*?)>\s*;", block, re.S)
    if not match:
        raise ValueError(f"property {name!r} not found")
    return match.group("body")


def zmk_kp_to_rmk(key: str) -> str:
    if key == "LG(TAB)":
        return "WM(Tab, LGui)"
    if key == "LG(D)":
        return "WM(D, LGui)"
    if key == "LC(LG(LEFT))":
        return "WM(Left, LCtrl|LGui)"
    if key == "LC(LG(RIGHT))":
        return "WM(Right, LCtrl|LGui)"
    try:
        return ZMK_KEY_TO_RMK[key]
    except KeyError as e:
        raise ValueError(f"unmapped ZMK key {key!r}") from e


def consume_zmk_binding(tokens: list[str], index: int) -> tuple[str, int]:
    behavior = tokens[index]
    if behavior == "&kp":
        return zmk_kp_to_rmk(tokens[index + 1]), index + 2
    if behavior == "&mt2":
        hold = zmk_kp_to_rmk(tokens[index + 1])
        tap = zmk_kp_to_rmk(tokens[index + 2])
        if hold == "LShift":
            return f"MT({tap}, {hold})", index + 3
        return f"TH({tap}, {hold})", index + 3
    if behavior == "&mo":
        return f"MO({tokens[index + 1]})", index + 2
    if behavior == "&none":
        return NO_KEY, index + 1
    if behavior == "&trans":
        return TRANSPARENT, index + 1
    if behavior == "&mkp":
        button = tokens[index + 1]
        try:
            return ZMK_MOUSE_TO_RMK[button], index + 2
        except KeyError as e:
            raise ValueError(f"unmapped ZMK mouse button {button!r}") from e
    if behavior == "&out":
        output = tokens[index + 1]
        if output == "OUT_TOG":
            return "User7", index + 2
        raise ValueError(f"unmapped ZMK output behavior {output!r}")
    if behavior == "&bt":
        command = tokens[index + 1]
        if command == "BT_SEL":
            return f"User{tokens[index + 2]}", index + 3
        if command == "BT_CLR":
            return "User6", index + 2
        if command == "BT_CLR_ALL":
            return "User8", index + 2
        raise ValueError(f"unmapped ZMK Bluetooth behavior {command!r}")
    if behavior == "&sys_reset":
        return "Reboot", index + 1
    if behavior == "&bootloader":
        return "Bootloader", index + 1
    if behavior == "&zip_dyn_scale":
        key = (tokens[index + 1], tokens[index + 2])
        try:
            return ZMK_DYNAMIC_SCALE_TO_RMK[key], index + 3
        except KeyError as e:
            raise ValueError(f"unmapped ZMK dynamic scale behavior {key!r}") from e
    raise ValueError(f"unmapped ZMK behavior {behavior!r}")


def parse_zmk_bindings(body: str) -> list[str]:
    tokens = body.split()
    bindings: list[str] = []
    index = 0
    while index < len(tokens):
        binding, index = consume_zmk_binding(tokens, index)
        bindings.append(binding)
    return bindings


def zmk_rows_from_bindings(bindings: list[str]) -> list[list[str]]:
    if len(bindings) != 68:
        raise ValueError(f"expected 68 ZMK bindings per layer, got {len(bindings)}")

    source_rows = [
        bindings[0:10],
        bindings[10:20],
        bindings[20:30],
        bindings[30:42],
        bindings[42:52],
        bindings[52:58],
        bindings[58:68],
    ]
    if bindings[68:]:
        raise ValueError(f"unexpected trailing ZMK bindings: {bindings[68:]!r}")

    return [
        source_rows[0][0:5] + [NO_KEY, NO_KEY] + source_rows[0][5:10],
        source_rows[1][0:5] + [NO_KEY, NO_KEY] + source_rows[1][5:10],
        source_rows[2][0:5] + [NO_KEY, NO_KEY] + source_rows[2][5:10],
        source_rows[3],
        [NO_KEY] + source_rows[4] + [NO_KEY],
        source_rows[5][0:3] + [NO_KEY] * 6 + source_rows[5][3:6],
        source_rows[6][0:5] + [NO_KEY, NO_KEY] + source_rows[6][5:10],
    ]


def apply_documented_rmk_deltas(
    manifest: dict[str, Any],
    rows: list[list[list[str]]],
) -> list[list[list[str]]]:
    copied = [[list(row) for row in layer] for layer in rows]
    for delta in manifest.get("source_deltas", []):
        copied[int(delta["layer"])][int(delta["row"])][int(delta["col"])] = delta["target_expected"]
    return copied


def raw_zmk_keymap_rows(path: Path) -> list[list[list[str]]]:
    text = path.read_text()
    layers: list[list[list[str]]] = []
    for layer_name in LAYER_NAMES:
        block = extract_block(text, layer_name)
        layers.append(zmk_rows_from_bindings(parse_zmk_bindings(extract_angle_property(block, "bindings"))))
    return layers


def manifest_keymap_rows(manifest: dict[str, Any]) -> list[list[list[str]]]:
    rows = [
        [[None for _ in range(manifest["layout"]["cols"])] for _ in range(manifest["layout"]["rows"])]
        for _ in range(manifest["layout"]["layers"])
    ]
    for row_check in manifest.get("keymap_rows", []):
        rows[int(row_check["layer"])][int(row_check["row"])] = list(row_check["expected"])
    missing = [
        f"l{layer}r{row}"
        for layer, layer_rows in enumerate(rows)
        for row, values in enumerate(layer_rows)
        if any(value is None for value in values)
    ]
    if missing:
        raise ValueError(f"manifest is missing keymap rows: {', '.join(missing)}")
    return rows  # type: ignore[return-value]


def zmk_combo_blocks(text: str) -> dict[str, tuple[list[int], str]]:
    combos_block = extract_block(text, "combos")
    combos: dict[str, tuple[list[int], str]] = {}
    for match in re.finditer(r"\b(COMBO_[A-Z0-9_]+|CONBO_[A-Z0-9_]+)\s*\{", combos_block):
        name = match.group(1)
        block_start = match.end() - 1
        depth = 0
        for index in range(block_start, len(combos_block)):
            if combos_block[index] == "{":
                depth += 1
            elif combos_block[index] == "}":
                depth -= 1
                if depth == 0:
                    block = combos_block[block_start + 1 : index]
                    break
        else:
            raise ValueError(f"combo block {name!r} is not closed")
        positions = [int(value) for value in extract_angle_property(block, "key-positions").split()]
        output = parse_zmk_bindings(extract_angle_property(block, "bindings"))
        if len(output) != 1:
            raise ValueError(f"combo {name!r} has multiple outputs: {output!r}")
        combos[name] = (positions, output[0])
    return combos


def zmk_flat_base_bindings(path: Path) -> list[str]:
    text = path.read_text()
    block = extract_block(text, "DEFAULT_LAYER")
    return parse_zmk_bindings(extract_angle_property(block, "bindings"))


def angle_int_property(block: str, name: str) -> int:
    return int(extract_angle_property(block, name).strip())


def scalar_property(block: str, name: str) -> str:
    match = re.search(rf"\b{re.escape(name)}\s*=\s*\"?(?P<value>[^\";\s]+)\"?\s*;", block)
    if not match:
        raise ValueError(f"property {name!r} not found")
    return match.group("value")


def check_zmk_source_deltas(manifest: dict[str, Any], raw_layers: list[list[list[str]]]) -> list[Result]:
    results: list[Result] = []
    for delta in manifest.get("source_deltas", []):
        layer = int(delta["layer"])
        row = int(delta["row"])
        col = int(delta["col"])
        actual = raw_layers[layer][row][col]
        expected = delta["source_expected"]
        ok = actual == expected
        results.append(
            Result(
                delta["id"],
                "zmk_source_delta",
                1 if ok else 0,
                1,
                "ok" if ok else f"raw source expected {expected!r}, got {actual!r}",
            )
        )
    return results


def check_zmk_behavior_source(manifest: dict[str, Any], source_text: str) -> list[Result]:
    results: list[Result] = []
    mt_block = extract_block(source_text, "&mt")
    lt_block = extract_block(source_text, "&lt")
    mt2_block = extract_block(source_text, "mt2")
    source_values = {
        "zmk_mt_quick_tap_ms": angle_int_property(mt_block, "quick-tap-ms"),
        "zmk_lt_quick_tap_ms": angle_int_property(lt_block, "quick-tap-ms"),
        "zmk_mt2_flavor": scalar_property(mt2_block, "flavor"),
        "zmk_mt2_tapping_term_ms": angle_int_property(mt2_block, "tapping-term-ms"),
        "zmk_mt2_quick_tap_ms": angle_int_property(mt2_block, "quick-tap-ms"),
        "zmk_mt2_require_prior_idle_ms": angle_int_property(mt2_block, "require-prior-idle-ms"),
    }
    for check in manifest.get("zmk_behavior_values", []):
        actual = source_values.get(check["id"])
        expected = check["expected"]
        ok = actual == expected
        results.append(
            Result(
                check["id"],
                "zmk_source_behavior",
                1 if ok else 0,
                1,
                f"expected {expected!r}, got {actual!r}",
            )
        )
    return results


def check_zmk_source(manifest: dict[str, Any], zmk_keymap_path: Path, required: bool) -> list[Result]:
    results: list[Result] = []
    if not zmk_keymap_path.exists():
        if required:
            return [
                Result(
                    "zmk_source.present",
                    "zmk_source",
                    0,
                    1,
                    f"missing ZMK keymap at {zmk_keymap_path}",
                )
            ]
        return [
            Result(
                "zmk_source.present",
                "zmk_source",
                0,
                0,
                f"skipped; ZMK keymap not found at {zmk_keymap_path}",
            )
        ]

    raw_source_layers = raw_zmk_keymap_rows(zmk_keymap_path)
    results.extend(check_zmk_source_deltas(manifest, raw_source_layers))
    source_layers = apply_documented_rmk_deltas(manifest, raw_source_layers)
    expected_layers = manifest_keymap_rows(manifest)
    for layer, (expected_layer, source_layer) in enumerate(zip(expected_layers, source_layers, strict=True)):
        for row, (expected, source) in enumerate(zip(expected_layer, source_layer, strict=True)):
            passed = 0
            mismatches: list[str] = []
            for col, (want, got) in enumerate(zip(expected, source, strict=True)):
                if want == got:
                    passed += 1
                else:
                    mismatches.append(f"c{col}: manifest {want!r}, source {got!r}")
            results.append(
                Result(
                    f"zmk_source.layer{layer}_row{row}",
                    "zmk_source_cell",
                    passed,
                    len(expected),
                    "ok" if not mismatches else "; ".join(mismatches),
                )
            )

    source_text = zmk_keymap_path.read_text()
    results.extend(check_zmk_behavior_source(manifest, source_text))
    source_combos = zmk_combo_blocks(source_text)
    base_bindings = zmk_flat_base_bindings(zmk_keymap_path)
    manifest_combos = combo_set_from_manifest(manifest)
    expected_combos: set[tuple[tuple[str, ...], str, int]] = set()
    for name, (positions, output) in sorted(source_combos.items()):
        actions = tuple(base_bindings[position] for position in positions)
        expected = (actions, output, 0)
        expected_combos.add(expected)
        ok = expected in manifest_combos
        results.append(
            Result(
                f"zmk_source.combo.{name}",
                "zmk_source_combo",
                1 if ok else 0,
                1,
                "ok" if ok else f"manifest missing {expected!r}",
            )
        )
    extra_combos = manifest_combos - expected_combos
    results.append(
        Result(
            "zmk_source.combo.no_extra_manifest_combos",
            "zmk_source_combo",
            1 if not extra_combos else 0,
            1,
            "ok" if not extra_combos else f"extra manifest combos {sorted(extra_combos)!r}",
        )
    )

    conditional_block = extract_block(source_text, "Cond_Syslayer")
    source_layers_match = (
        extract_angle_property(conditional_block, "if-layers").split() == ["1", "2"]
        and extract_angle_property(conditional_block, "then-layer").split() == ["3"]
    )
    behavior_values = {check["id"]: check["expected"] for check in manifest.get("behavior_values", [])}
    manifest_layers_match = (
        behavior_values.get("tri_layer_lower") == 1
        and behavior_values.get("tri_layer_upper") == 2
        and behavior_values.get("tri_layer_adjust") == 3
    )
    ok = source_layers_match and manifest_layers_match
    results.append(
        Result(
            "zmk_source.conditional_layer",
            "zmk_source_behavior",
            1 if ok else 0,
            1,
            "ok" if ok else "ZMK conditional layer 1+2=>3 is not mirrored by manifest tri-layer",
        )
    )
    return results


def combo_set_from_manifest(manifest: dict[str, Any]) -> set[tuple[tuple[str, ...], str, int]]:
    return {
        (tuple(combo["actions"]), combo["output"], int(combo["layer"]))
        for combo in manifest.get("combos", [])
    }


def default_zmk_keymap_path(manifest: dict[str, Any]) -> Path:
    hint = manifest.get("metadata", {}).get("source_repo_hint")
    if hint:
        return Path(hint) / "config" / "lalapadgen2.keymap"
    return Path("/nonexistent/zmk-keymap")


def run(
    manifest_path: Path,
    keyboard_path: Path,
    zmk_keymap_path: Path | None,
    require_zmk_source: bool,
) -> list[Result]:
    manifest = load_toml(manifest_path)
    keyboard = load_toml(keyboard_path)
    results: list[Result] = []
    results.extend(check_layout(manifest, keyboard))
    results.extend(check_keymap_rows(manifest, keyboard))
    results.extend(check_behavior_values(manifest, keyboard))
    results.extend(check_combos(manifest, keyboard))
    results.extend(check_scenarios(manifest, keyboard))
    results.extend(
        check_zmk_source(
            manifest,
            zmk_keymap_path if zmk_keymap_path is not None else default_zmk_keymap_path(manifest),
            require_zmk_source,
        )
    )
    return results


def print_text(results: list[Result]) -> None:
    passed = sum(result.passed for result in results)
    total = sum(result.total for result in results)
    rate = 100.0 if total == 0 else passed * 100.0 / total
    print(f"Porting coverage: {passed}/{total} = {rate:.2f}%")
    for result in results:
        status = "SKIP" if result.total == 0 else "ok" if result.ok else "FAIL"
        print(f"{status:4} {result.kind:12} {result.id}: {result.passed}/{result.total} {result.message}")


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=Path("tools/porting_coverage_manifest.toml"))
    parser.add_argument("--keyboard-toml", type=Path, default=Path("keyboard.toml"))
    parser.add_argument(
        "--zmk-keymap",
        type=Path,
        default=None,
        help="Optional path to the upstream ZMK config/lalapadgen2.keymap.",
    )
    parser.add_argument(
        "--require-zmk-source",
        action="store_true",
        help="Fail if the upstream ZMK keymap cannot be read.",
    )
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)

    results = run(args.manifest, args.keyboard_toml, args.zmk_keymap, args.require_zmk_source)
    passed = sum(result.passed for result in results)
    total = sum(result.total for result in results)
    if args.json:
        print(
            json.dumps(
                {
                    "passed": passed,
                    "total": total,
                    "rate": None if total == 0 else passed / total,
                    "results": [result.__dict__ | {"ok": result.ok} for result in results],
                },
                indent=2,
                sort_keys=True,
            )
        )
    else:
        print_text(results)
    return 0 if passed == total else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
