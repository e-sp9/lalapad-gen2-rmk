#!/usr/bin/env python3
"""Measure ZMK-to-RMK porting coverage from a manifest.

The manifest is the migration contract. It is intentionally explicit: each
keymap row, behavior value, combo, and high-risk thumb-layer scenario has an
expected RMK result derived from the upstream ZMK implementation plus documented
RMK-specific deltas.
"""

from __future__ import annotations

import argparse
import hashlib
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

XIAO_D_TO_NRF = {
    0: "P0_02",
    1: "P0_03",
    2: "P0_28",
    3: "P0_29",
    4: "P0_04",
    5: "P0_05",
    6: "P1_11",
    7: "P1_12",
    8: "P1_13",
    9: "P1_14",
    10: "P1_15",
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


def load_json(path: Path) -> Any:
    return json.loads(path.read_text())


def path_get(root: dict[str, Any], dotted: str) -> Any:
    value: Any = root
    for part in dotted.split("."):
        if isinstance(value, list) and part.isdigit():
            index = int(part)
            if index >= len(value):
                raise KeyError(dotted)
            value = value[index]
            continue
        if not isinstance(value, dict) or part not in value:
            raise KeyError(dotted)
        value = value[part]
    return value


def keymap(config: dict[str, Any]) -> list[list[list[str]]]:
    return config["layout"]["keymap"]


def combo_list(config: dict[str, Any]) -> list[tuple[tuple[str, ...], str, int]]:
    combos = config["behavior"]["combo"]["combos"]
    return [
        (tuple(combo["actions"]), combo["output"], int(combo["layer"]))
        for combo in combos
    ]


def combo_set(config: dict[str, Any]) -> set[tuple[tuple[str, ...], str, int]]:
    return set(combo_list(config))


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
    return resolve_key_from_layers(km, row, col, active_layers)


def resolve_key_from_layers(
    layers: list[list[list[str]]], row: int, col: int, active_layers: list[int]
) -> str:
    for layer in sorted(active_layers, reverse=True):
        action = layers[layer][row][col]
        if action != TRANSPARENT:
            return tap_action(action)
    return tap_action(layers[0][row][col])


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


def check_keymap_shape(manifest: dict[str, Any], config: dict[str, Any]) -> list[Result]:
    expected_layers = int(manifest["layout"]["layers"])
    expected_rows = int(manifest["layout"]["rows"])
    expected_cols = int(manifest["layout"]["cols"])
    km = keymap(config)

    passed = 0
    total = 0
    messages: list[str] = []

    total += 1
    if len(km) == expected_layers:
        passed += 1
    else:
        messages.append(f"layers expected {expected_layers}, got {len(km)}")

    for layer in range(expected_layers):
        if layer >= len(km):
            total += 1 + expected_rows
            messages.append(f"layer {layer} missing")
            continue

        layer_rows = km[layer]
        total += 1
        if len(layer_rows) == expected_rows:
            passed += 1
        else:
            messages.append(f"layer {layer} rows expected {expected_rows}, got {len(layer_rows)}")

        for row in range(expected_rows):
            total += 1
            if row >= len(layer_rows):
                messages.append(f"layer {layer} row {row} missing")
                continue

            actual_cols = len(layer_rows[row])
            if actual_cols == expected_cols:
                passed += 1
            else:
                messages.append(
                    f"layer {layer} row {row} cols expected {expected_cols}, got {actual_cols}"
                )

    return [
        Result(
            "keymap_shape_matches_layout",
            "keymap_shape",
            passed,
            total,
            "ok" if not messages else "; ".join(messages[:8]),
        )
    ]


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


def check_config_values(manifest: dict[str, Any], config: dict[str, Any]) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("config_values", []):
        expected = check["expected"]
        try:
            actual = path_get(config, check["path"])
        except KeyError:
            actual = None
        ok = actual == expected
        results.append(
            Result(
                check["id"],
                "config",
                1 if ok else 0,
                1,
                f"expected {expected!r}, got {actual!r}",
            )
        )
    return results


def check_combos(manifest: dict[str, Any], config: dict[str, Any]) -> list[Result]:
    results: list[Result] = []
    actual_list = combo_list(config)
    actual = set(actual_list)
    expected_all = combo_set_from_manifest(manifest)
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

    missing = expected_all - actual
    extra = actual - expected_all
    duplicates = sorted(
        combo
        for combo in actual
        if actual_list.count(combo) > 1
    )
    passed = len(expected_all) - len(missing)
    total = len(expected_all) + len(extra) + len(duplicates)
    messages: list[str] = []
    if missing:
        messages.append(f"missing combos {sorted(missing)!r}")
    if extra:
        messages.append(f"unexpected RMK combos {sorted(extra)!r}")
    if duplicates:
        messages.append(f"duplicated RMK combos {duplicates!r}")
    results.append(
        Result(
            "rmk_combo_set_matches_manifest",
            "combo_inventory",
            passed,
            total,
            "ok" if not messages else "; ".join(messages),
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


def check_zmk_source_scenarios(
    manifest: dict[str, Any],
    config: dict[str, Any],
    source_layers: list[list[list[str]]],
) -> list[Result]:
    results: list[Result] = []
    for scenario in manifest.get("scenarios", []):
        holds = list(scenario.get("holds", []))
        if "hold" in scenario:
            holds.append(scenario["hold"])

        passed = 0
        total = 1 + len(holds)
        messages: list[str] = []
        for hold in holds:
            action = source_layers[0][int(hold["row"])][int(hold["col"])]
            expected_action = hold["expected_action"]
            if action == expected_action:
                passed += 1
            else:
                messages.append(
                    f"source hold action expected {expected_action!r}, got {action!r}"
                )

        active_layers = active_layers_from_holds(config, holds)
        tap = scenario["tap"]
        output = resolve_key_from_layers(
            source_layers, int(tap["row"]), int(tap["col"]), active_layers
        )
        expected_output = scenario["expected_output"]
        if output == expected_output:
            passed += 1
        else:
            messages.append(f"source output expected {expected_output!r}, got {output!r}")

        results.append(
            Result(
                f"zmk_source.scenario.{scenario['id']}",
                "zmk_source_scenario",
                passed,
                total,
                "ok" if not messages else "; ".join(messages),
            )
        )
    return results


def check_zmk_source_layer_resolution(
    config: dict[str, Any],
    source_layers: list[list[list[str]]],
) -> list[Result]:
    layer_sets = [
        ("layer1_resolution", [1]),
        ("layer2_resolution", [2]),
        ("tri_layer_resolution", [1, 2, 3]),
    ]
    rows = int(config["layout"]["rows"])
    cols = int(config["layout"]["cols"])
    results: list[Result] = []

    for name, active_layers in layer_sets:
        passed = 0
        total = rows * cols
        messages: list[str] = []
        for row in range(rows):
            for col in range(cols):
                expected = resolve_key_from_layers(source_layers, row, col, active_layers)
                actual = resolve_key(config, row, col, active_layers)
                if actual == expected:
                    passed += 1
                else:
                    messages.append(
                        f"r{row}c{col}: source expected {expected!r}, RMK got {actual!r}"
                    )
        results.append(
            Result(
                f"zmk_source.{name}",
                "zmk_source_layer_resolution",
                passed,
                total,
                "ok" if not messages else "; ".join(messages[:8]),
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


def matching_block_end(text: str, start: int) -> int:
    depth = 0
    in_string = False
    escaped = False
    for index in range(start, len(text)):
        char = text[index]
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            continue

        if char == '"':
            in_string = True
        elif char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return index
    raise ValueError("block is not closed")


def strip_c_style_comments(text: str) -> str:
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    return re.sub(r"//.*", "", text)


def extract_angle_property(block: str, name: str) -> str:
    match = re.search(rf"\b{re.escape(name)}\s*=\s*<(?P<body>.*?)>\s*;", block, re.S)
    if not match:
        raise ValueError(f"property {name!r} not found")
    return match.group("body")


def extract_ref_property(block: str, name: str) -> str:
    pattern = (
        rf"(?<![A-Za-z0-9_,]){re.escape(name)}(?![A-Za-z0-9_,])\s*=\s*"
        r"(?:<(?P<angle>.*?)>|(?P<plain>&[A-Za-z0-9_]+))\s*;"
    )
    match = re.search(
        pattern,
        block,
        re.S,
    )
    if not match:
        raise ValueError(f"reference property {name!r} not found")
    return (match.group("angle") or match.group("plain")).strip()


def extract_top_level_property_body(block: str, name: str) -> str:
    match = re.search(
        rf"(?<![A-Za-z0-9_,#-]){re.escape(name)}(?![A-Za-z0-9_,#-])\s*=\s*(?P<body>.*?);",
        top_level_text(block),
        re.S,
    )
    if not match:
        raise ValueError(f"property {name!r} not found")
    return match.group("body")


def parse_angle_array_property(block: str, name: str) -> list[str]:
    body = extract_top_level_property_body(block, name)
    return [" ".join(match.group(1).split()) for match in re.finditer(r"<\s*(.*?)\s*>", body)]


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


def zmk_rows_from_bindings(
    bindings: list[str],
    transform_rows: int,
    transform_cols: int,
    transform_positions: list[tuple[int, int]],
) -> list[list[str]]:
    if len(bindings) != len(transform_positions):
        raise ValueError(
            f"expected {len(transform_positions)} ZMK bindings per layer from default_transform, "
            f"got {len(bindings)}"
        )

    rows = [[NO_KEY for _ in range(transform_cols)] for _ in range(transform_rows)]
    for binding, (row, col) in zip(bindings, transform_positions, strict=True):
        if row >= transform_rows or col >= transform_cols:
            raise ValueError(f"default_transform position {(row, col)!r} exceeds matrix shape")
        if rows[row][col] != NO_KEY:
            raise ValueError(f"default_transform maps duplicate position {(row, col)!r}")
        rows[row][col] = binding
    return rows


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
    transform_rows, transform_cols, transform_positions = zmk_matrix_transform(
        path.parent / "boards/shields/lalapadgen2/lalapadgen2.dtsi"
    )
    layers: list[list[list[str]]] = []
    for layer_name in LAYER_NAMES:
        block = extract_block(text, layer_name)
        layers.append(
            zmk_rows_from_bindings(
                parse_zmk_bindings(extract_angle_property(block, "bindings")),
                transform_rows,
                transform_cols,
                transform_positions,
            )
        )
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


def top_level_text(block: str) -> str:
    chars: list[str] = []
    depth = 0
    in_string = False
    escaped = False
    for char in block:
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            chars.append(char if depth == 0 else " ")
        elif char == '"':
            in_string = True
            chars.append(char if depth == 0 else " ")
        elif char == "{":
            depth += 1
            chars.append(" ")
        elif char == "}":
            if depth > 0:
                depth -= 1
            chars.append(" ")
        elif depth == 0:
            chars.append(char)
        else:
            chars.append(" ")
    return "".join(chars)


def has_top_level_property(block: str, name: str) -> bool:
    return re.search(rf"(?<![A-Za-z0-9_,]){re.escape(name)}\s*=", top_level_text(block)) is not None


def top_level_child_blocks(block: str) -> list[tuple[str | None, str, str]]:
    children: list[tuple[str | None, str, str]] = []
    index = 0
    node_name = r"[A-Za-z0-9,._+\-]+(?:@[A-Za-z0-9,._+\-]+)?"
    pattern = re.compile(
        r"(?:^|[;\n])\s*"
        r"(?:(?P<label>[A-Za-z_][A-Za-z0-9_]*)\s*:\s*)?"
        rf"(?P<name>{node_name})\s*\{{"
    )
    while match := pattern.search(block, index):
        body_start = match.end() - 1
        body_end = matching_block_end(block, body_start)
        body = block[body_start + 1 : body_end]
        children.append((match.group("label"), match.group("name"), body))
        index = body_end + 1
    return children


def zmk_keymap_layer_names(source_text: str) -> list[str]:
    keymap_block = extract_block(strip_c_style_comments(source_text), "keymap")
    names: list[str] = []
    for _, name, body in top_level_child_blocks(keymap_block):
        if has_top_level_property(body, "bindings"):
            names.append(name)
    return names


def check_zmk_keymap_layer_inventory(manifest: dict[str, Any], source_text: str) -> list[Result]:
    expected = list(manifest.get("source_inventory", {}).get("keymap_layers", LAYER_NAMES))
    actual = zmk_keymap_layer_names(source_text)
    total = max(len(expected), len(actual))
    passed = 0
    mismatches: list[str] = []
    for index in range(total):
        want = expected[index] if index < len(expected) else None
        got = actual[index] if index < len(actual) else None
        if want == got:
            passed += 1
        else:
            mismatches.append(f"l{index}: expected {want!r}, got {got!r}")
    return [
        Result(
            "zmk_source.keymap_layer_inventory",
            "zmk_source_inventory",
            passed,
            total,
            "ok" if not mismatches else "; ".join(mismatches),
        )
    ]


def zmk_behavior_node_names(source_text: str) -> list[str]:
    behaviors_block = extract_block(strip_c_style_comments(source_text), "behaviors")
    return [label or name for label, name, _ in top_level_child_blocks(behaviors_block)]


def check_zmk_behavior_inventory(manifest: dict[str, Any], source_text: str) -> list[Result]:
    expected = list(manifest.get("source_inventory", {}).get("behavior_nodes", []))
    actual_list = zmk_behavior_node_names(source_text)
    actual = set(actual_list)
    expected_set = set(expected)
    missing = sorted(expected_set - actual)
    extra = sorted(actual - expected_set)
    duplicates = sorted(node for node in actual if actual_list.count(node) > 1)
    passed = len(expected_set) - len(missing)
    total = len(expected_set) + len(extra) + len(duplicates)
    messages: list[str] = []
    if missing:
        messages.append(f"missing behavior nodes {missing!r}")
    if extra:
        messages.append(f"unexpected behavior nodes {extra!r}")
    if duplicates:
        messages.append(f"duplicated behavior nodes {duplicates!r}")
    return [
        Result(
            "zmk_source.behavior_inventory",
            "zmk_source_inventory",
            passed,
            total,
            "ok" if not messages else "; ".join(messages),
        )
    ]


def check_zmk_behavior_property_inventory(
    manifest: dict[str, Any], zmk_config_dir: Path
) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("source_inventory", {}).get("behavior_properties", []):
        source_file = check["source_file"]
        expected = list(check["expected"])
        source_path = zmk_config_dir / source_file
        result_id = f"zmk_source.behavior_properties.{check['source_block']}"
        if not source_path.exists():
            results.append(
                Result(
                    result_id,
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"missing behavior property source file {source_file!r}",
                )
            )
            continue
        try:
            actual = dts_property_inventory(source_path.read_text(), check["source_block"])
        except ValueError as e:
            results.append(
                Result(
                    result_id,
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"invalid behavior property source {source_file!r}: {e}",
                )
            )
            continue
        results.append(ordered_inventory_result(result_id, "zmk_inventory", expected, actual))
    return results


def check_zmk_combo_property_inventory(
    manifest: dict[str, Any], zmk_config_dir: Path
) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("source_inventory", {}).get("combo_properties", []):
        source_file = check["source_file"]
        expected = list(check["expected"])
        source_path = zmk_config_dir / source_file
        result_id = f"zmk_source.combo_properties.{check['source_block']}"
        if not source_path.exists():
            results.append(
                Result(
                    result_id,
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"missing combo property source file {source_file!r}",
                )
            )
            continue
        try:
            actual = dts_property_inventory(source_path.read_text(), check["source_block"])
        except ValueError as e:
            results.append(
                Result(
                    result_id,
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"invalid combo property source {source_file!r}: {e}",
                )
            )
            continue
        results.append(ordered_inventory_result(result_id, "zmk_inventory", expected, actual))
    return results


def zmk_combo_blocks(text: str) -> dict[str, tuple[list[int], str]]:
    combos_block = extract_block(strip_c_style_comments(text), "combos")
    combos: dict[str, tuple[list[int], str]] = {}
    for label, node_name, block in top_level_child_blocks(combos_block):
        if not (
            has_top_level_property(block, "key-positions")
            and has_top_level_property(block, "bindings")
        ):
            continue
        name = label or node_name
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


def top_level_scalar_property(block: str, name: str) -> str | None:
    match = re.search(
        rf"(?<![A-Za-z0-9_,]){re.escape(name)}\s*=\s*\"?(?P<value>[^\";\s]+)\"?\s*;",
        top_level_text(block),
    )
    return match.group("value") if match else None


def dts_child_blocks(block: str) -> list[tuple[str | None, str, str]]:
    children: list[tuple[str | None, str, str]] = []
    index = 0
    node_name = r"&?[A-Za-z0-9,._+\-]+(?:@[A-Za-z0-9,._+\-]+)?"
    pattern = re.compile(
        r"(?:^|[;\n])\s*"
        r"(?:(?P<label>[A-Za-z_][A-Za-z0-9_]*)\s*:\s*)?"
        rf"(?P<name>{node_name})\s*\{{"
    )
    while match := pattern.search(block, index):
        body_start = match.end() - 1
        body_end = matching_block_end(block, body_start)
        body = block[body_start + 1 : body_end]
        children.append((match.group("label"), match.group("name"), body))
        index = body_end + 1
    return children


def dts_status_nodes(text: str) -> list[tuple[str, str]]:
    found: list[tuple[str, str]] = []

    def visit(block: str) -> None:
        for label, name, body in dts_child_blocks(block):
            node = label or name
            if status := top_level_scalar_property(body, "status"):
                found.append((node, status))
            visit(body)

    visit(strip_c_style_comments(text))
    return found


def zmk_pin_to_rmk(controller: str, pin: int) -> str:
    if controller == "xiao_d":
        try:
            return XIAO_D_TO_NRF[pin]
        except KeyError as e:
            raise ValueError(f"unmapped XIAO D pin {pin}") from e
    if controller == "gpio0":
        return f"P0_{pin:02d}"
    if controller == "gpio1":
        return f"P1_{pin:02d}"
    raise ValueError(f"unmapped GPIO controller {controller!r}")


def parse_gpio_property(block: str, name: str) -> list[str]:
    body = f"<{extract_angle_property(block, name)}>"
    return [
        zmk_pin_to_rmk(match.group(1), int(match.group(2)))
        for match in re.finditer(r"<&([A-Za-z0-9_]+)\s+([0-9]+)\b[^>]*>", body)
    ]


def normalize_gpio_flags(flags: str) -> str:
    flags = flags.strip()
    if flags.startswith("(") and flags.endswith(")"):
        flags = flags[1:-1]
    return re.sub(r"\s*\|\s*", "|", " ".join(flags.split()))


def parse_gpio_property_with_flags(block: str, name: str) -> list[str]:
    body = f"<{extract_angle_property(block, name)}>"
    entries: list[str] = []
    for match in re.finditer(r"<&([A-Za-z0-9_]+)\s+([0-9]+)\s+([^>]+)>", body):
        entries.append(
            f"{match.group(1)}:{int(match.group(2))}:{normalize_gpio_flags(match.group(3))}"
        )
    return entries


def parse_kconfig(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    text = path.read_text()
    for match in re.finditer(r"(?m)^\s*(CONFIG_[A-Za-z0-9_]+)=([^\s#]+)", text):
        values[match.group(1)] = match.group(2)
    return values


def active_kconfig_line_inventory(text: str) -> list[str]:
    return [
        match.group(0).strip()
        for match in re.finditer(r"(?m)^\s*CONFIG_[A-Za-z0-9_]+=[^\s#]+", text)
    ]


def disabled_kconfig_line_inventory(text: str) -> list[str]:
    return [
        match.group(1).strip()
        for match in re.finditer(r"(?m)^\s*#\s*(CONFIG_[A-Za-z0-9_]+=[^\s#]+)", text)
    ]


def check_zmk_kconfig_line_inventory(
    manifest: dict[str, Any], zmk_config_dir: Path
) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("source_inventory", {}).get("kconfig_lines", []):
        source_file = check["source_file"]
        expected = list(check["expected"])
        source_path = zmk_config_dir / source_file
        result_id = f"zmk_source.kconfig_lines.{source_file}"
        if not source_path.exists():
            results.append(
                Result(
                    result_id,
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"missing ZMK Kconfig source file {source_file!r}",
                )
            )
            continue
        actual = active_kconfig_line_inventory(source_path.read_text())
        results.append(ordered_inventory_result(result_id, "zmk_inventory", expected, actual))
    return results


def check_zmk_disabled_kconfig_line_inventory(
    manifest: dict[str, Any], zmk_config_dir: Path
) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("source_inventory", {}).get("disabled_kconfig_lines", []):
        source_file = check["source_file"]
        expected = list(check["expected"])
        source_path = zmk_config_dir / source_file
        result_id = f"zmk_source.disabled_kconfig_lines.{source_file}"
        if not source_path.exists():
            results.append(
                Result(
                    result_id,
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"missing ZMK Kconfig source file {source_file!r}",
                )
            )
            continue
        actual = disabled_kconfig_line_inventory(source_path.read_text())
        results.append(ordered_inventory_result(result_id, "zmk_inventory", expected, actual))
    return results


def parse_rust_const(path: Path, name: str) -> Any:
    text = re.sub(r"/\*.*?\*/", "", path.read_text(), flags=re.S)
    text = re.sub(r"//.*", "", text)
    match = re.search(
        rf"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?const\s+{re.escape(name)}\s*:[^=]+=\s*(?P<value>[^;]+);",
        text,
    )
    if not match:
        raise ValueError(f"const {name!r} not found in {path}")
    value = match.group("value").strip()
    if value == "true":
        return True
    if value == "false":
        return False
    if value.startswith("0x"):
        return int(value, 16)
    numeric = re.sub(r"_(?=\d)", "", value)
    numeric = re.sub(r"(u|i)(8|16|32|64|128|size)$", "", numeric)
    if re.fullmatch(r"-?\d+", numeric):
        return int(numeric)
    return value


def parse_rust_byte_array(path: Path, name: str) -> list[int]:
    text = re.sub(r"/\*.*?\*/", "", path.read_text(), flags=re.S)
    text = re.sub(r"//.*", "", text)
    match = re.search(
        rf"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?const\s+{re.escape(name)}\s*:\s*\[u8;\s*(?P<len>\d+)\]\s*=\s*\[(?P<body>.*?)\];",
        text,
        re.S,
    )
    if not match:
        raise ValueError(f"byte array const {name!r} not found in {path}")
    values = [
        int(value, 16) if value.lower().startswith("0x") else int(value)
        for value in re.findall(r"0x[0-9a-fA-F]+|\b\d+\b", match.group("body"))
    ]
    declared_len = int(match.group("len"))
    if len(values) != declared_len:
        raise ValueError(
            f"byte array const {name!r} length mismatch in {path}: declared {declared_len}, got {len(values)}"
        )
    return values


def check_source_regex_values(manifest: dict[str, Any], zmk_config_dir: Path) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("source_regex_values", []):
        source_path = zmk_config_dir / check["source_file"]
        text = source_path.read_text()
        ok = re.search(check["pattern"], text, re.S) is not None
        results.append(
            Result(
                check["id"],
                "zmk_source_regex",
                1 if ok else 0,
                1,
                "ok" if ok else f"pattern {check['pattern']!r} not found in {source_path}",
            )
        )
    return results


def check_zmk_config_values(
    manifest: dict[str, Any],
    keyboard: dict[str, Any],
    zmk_config_dir: Path,
) -> list[Result]:
    results: list[Result] = []
    cache: dict[Path, dict[str, str]] = {}
    for check in manifest.get("zmk_config_values", []):
        source_path = zmk_config_dir / check["source_file"]
        if source_path not in cache:
            cache[source_path] = parse_kconfig(source_path)
        actual_source = cache[source_path].get(check["key"])
        expected_source = check["source_expected"]
        source_ok = actual_source == expected_source

        passed = 1 if source_ok else 0
        total = 1
        messages = [f"source expected {expected_source!r}, got {actual_source!r}"]

        if "target_path" in check:
            try:
                actual_target = path_get(keyboard, check["target_path"])
            except KeyError:
                actual_target = None
            expected_target = check["target_expected"]
            target_ok = actual_target == expected_target
            passed += 1 if target_ok else 0
            total += 1
            messages.append(f"target expected {expected_target!r}, got {actual_target!r}")

        results.append(
            Result(
                check["id"],
                "zmk_config",
                passed,
                total,
                "ok" if passed == total else "; ".join(messages),
            )
        )
    return results


def check_zmk_config_mirrors(manifest: dict[str, Any], zmk_config_dir: Path) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("zmk_config_mirrors", []):
        source_files = [zmk_config_dir / source_file for source_file in check["source_files"]]
        configs = [parse_kconfig(source_file) for source_file in source_files]
        passed = 0
        total = 0
        messages: list[str] = []

        for key in check["keys"]:
            total += 1
            values = [config.get(key) for config in configs]
            if all(value is not None for value in values) and len(set(values)) == 1:
                passed += 1
            else:
                joined = ", ".join(
                    f"{source_file.name}={value!r}"
                    for source_file, value in zip(source_files, values, strict=True)
                )
                messages.append(f"{key}: {joined}")

        results.append(
            Result(
                check["id"],
                "zmk_config_mirror",
                passed,
                total,
                "ok" if not messages else "; ".join(messages),
            )
        )
    return results


def covered_zmk_config_keys(manifest: dict[str, Any]) -> dict[str, set[str]]:
    covered: dict[str, set[str]] = {}

    def add(source_file: str, key: str) -> None:
        covered.setdefault(source_file, set()).add(key)

    for check in manifest.get("zmk_config_values", []):
        add(check["source_file"], check["key"])
    for check in manifest.get("rust_const_values", []):
        if "source_file" in check and "source_key" in check:
            add(check["source_file"], check["source_key"])
    for check in manifest.get("zmk_config_mirrors", []):
        for source_file in check["source_files"]:
            for key in check["keys"]:
                add(source_file, key)

    return covered


def check_zmk_config_inventory(manifest: dict[str, Any], zmk_config_dir: Path) -> list[Result]:
    inventory = manifest.get("source_inventory", {})
    covered = covered_zmk_config_keys(manifest)
    results: list[Result] = []
    for source_file in inventory.get("kconfig_files", []):
        actual_keys = set(parse_kconfig(zmk_config_dir / source_file))
        covered_keys = covered.get(source_file, set())
        missing = sorted(actual_keys - covered_keys)
        extra = sorted(covered_keys - actual_keys)
        passed = len(actual_keys) - len(missing)
        total = len(actual_keys) + len(extra)
        messages: list[str] = []
        if missing:
            messages.append(f"unclassified active keys {missing!r}")
        if extra:
            messages.append(f"manifest references absent keys {extra!r}")
        results.append(
            Result(
                f"kconfig_inventory.{source_file}",
                "zmk_inventory",
                passed,
                total,
                "ok" if not messages else "; ".join(messages),
            )
        )
    return results


def check_zmk_source_file_inventory(manifest: dict[str, Any], zmk_config_dir: Path) -> list[Result]:
    expected = list(manifest.get("source_inventory", {}).get("source_files", []))

    def is_source_file(path: Path) -> bool:
        relative = path.relative_to(zmk_config_dir)
        if any(part.startswith(".") for part in relative.parts):
            return False
        return path.name.startswith("Kconfig") or path.suffix in {
            ".conf",
            ".dtsi",
            ".json",
            ".keymap",
            ".overlay",
            ".yml",
        }

    actual_list = sorted(
        path.relative_to(zmk_config_dir).as_posix()
        for path in zmk_config_dir.rglob("*")
        if path.is_file() and is_source_file(path)
    )
    actual = set(actual_list)
    expected_set = set(expected)
    missing = sorted(expected_set - actual)
    extra = sorted(actual - expected_set)
    duplicates = sorted(item for item in actual if actual_list.count(item) > 1)
    passed = len(expected_set) - len(missing)
    total = len(expected_set) + len(extra) + len(duplicates)
    messages: list[str] = []
    if missing:
        messages.append(f"missing source files {missing!r}")
    if extra:
        messages.append(f"unclassified source files {extra!r}")
    if duplicates:
        messages.append(f"duplicated source files {duplicates!r}")
    return [
        Result(
            "zmk_source.file_inventory",
            "zmk_inventory",
            passed,
            total,
            "ok" if not messages else "; ".join(messages),
        )
    ]


def check_zmk_repo_file_inventory(manifest: dict[str, Any], zmk_config_dir: Path) -> list[Result]:
    expected = list(manifest.get("source_inventory", {}).get("repo_files", []))
    if not expected:
        return []

    repo_root = zmk_config_dir.parent if zmk_config_dir.name == "config" else zmk_config_dir
    ignored_dirs = {".git", ".west", "__pycache__"}
    ignored_files = {".DS_Store"}
    actual_list = sorted(
        path.relative_to(repo_root).as_posix()
        for path in repo_root.rglob("*")
        if path.is_file()
        and not any(part in ignored_dirs for part in path.relative_to(repo_root).parts)
        and path.name not in ignored_files
    )
    actual = set(actual_list)
    expected_set = set(expected)
    missing = sorted(expected_set - actual)
    extra = sorted(actual - expected_set)
    duplicates = sorted(item for item in actual if actual_list.count(item) > 1)
    passed = len(expected_set) - len(missing)
    total = len(expected_set) + len(extra) + len(duplicates)
    messages: list[str] = []
    if missing:
        messages.append(f"missing repo files {missing!r}")
    if extra:
        messages.append(f"unclassified repo files {extra!r}")
    if duplicates:
        messages.append(f"duplicated repo files {duplicates!r}")
    return [
        Result(
            "zmk_source.repo_file_inventory",
            "zmk_inventory",
            passed,
            total,
            "ok" if not messages else "; ".join(messages),
        )
    ]


def zmk_include_targets(text: str) -> list[str]:
    return [
        match.group("target")
        for match in re.finditer(
            r"(?m)^\s*#include\s+(?P<target><[^>]+>|\"[^\"]+\")",
            strip_c_style_comments(text),
        )
    ]


def check_zmk_include_inventory(manifest: dict[str, Any], zmk_config_dir: Path) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("source_inventory", {}).get("include_files", []):
        source_file = check["source_file"]
        expected = list(check["expected"])
        source_path = zmk_config_dir / source_file
        if not source_path.exists():
            results.append(
                Result(
                    f"zmk_source.include_inventory.{source_file}",
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"missing include source file {source_file!r}",
                )
            )
            continue
        actual = zmk_include_targets(source_path.read_text())
        total = max(len(expected), len(actual))
        passed = 0
        mismatches: list[str] = []
        for index in range(total):
            want = expected[index] if index < len(expected) else None
            got = actual[index] if index < len(actual) else None
            if want == got:
                passed += 1
            else:
                mismatches.append(f"i{index}: expected {want!r}, got {got!r}")
        results.append(
            Result(
                f"zmk_source.include_inventory.{source_file}",
                "zmk_inventory",
                passed,
                total,
                "ok" if not mismatches else "; ".join(mismatches),
            )
        )
    return results


def ordered_inventory_result(
    result_id: str,
    kind: str,
    expected: list[str],
    actual: list[str],
) -> Result:
    total = max(len(expected), len(actual))
    passed = 0
    mismatches: list[str] = []
    for index in range(total):
        want = expected[index] if index < len(expected) else None
        got = actual[index] if index < len(actual) else None
        if want == got:
            passed += 1
        else:
            mismatches.append(f"i{index}: expected {want!r}, got {got!r}")
    return Result(
        result_id,
        kind,
        passed,
        total,
        "ok" if not mismatches else "; ".join(mismatches),
    )


def kconfig_entry_inventory(text: str) -> list[str]:
    entries: list[str] = []
    current: str | None = None
    emitted_current = False
    for raw_line in text.splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        if match := re.fullmatch(r"config\s+([A-Za-z0-9_]+)", line):
            if current is not None and not emitted_current:
                entries.append(f"{current}:defined")
            current = match.group(1)
            emitted_current = False
            continue
        if current is None:
            continue
        if match := re.fullmatch(r"(default|def_bool)\s+(.+)", line):
            entries.append(f"{current}:{match.group(1)} {match.group(2).strip()}")
            emitted_current = True
    if current is not None and not emitted_current:
        entries.append(f"{current}:defined")
    return entries


def check_zmk_kconfig_entry_inventory(manifest: dict[str, Any], zmk_config_dir: Path) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("source_inventory", {}).get("kconfig_entries", []):
        source_file = check["source_file"]
        expected = list(check["expected"])
        source_path = zmk_config_dir / source_file
        if not source_path.exists():
            results.append(
                Result(
                    f"zmk_source.kconfig_entries.{source_file}",
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"missing Kconfig source file {source_file!r}",
                )
            )
            continue
        actual = kconfig_entry_inventory(source_path.read_text())
        results.append(
            ordered_inventory_result(
                f"zmk_source.kconfig_entries.{source_file}",
                "zmk_inventory",
                expected,
                actual,
            )
        )
    return results


def define_entry_inventory(text: str, prefix: str = "") -> list[str]:
    entries: list[str] = []
    for raw_line in strip_c_style_comments(text).splitlines():
        if match := re.fullmatch(r"\s*#define\s+([A-Za-z_][A-Za-z0-9_]*)\s+(.+?)\s*", raw_line):
            name = match.group(1)
            if prefix and not name.startswith(prefix):
                continue
            entries.append(f"{name}={match.group(2).strip()}")
    return entries


def check_zmk_define_entry_inventory(manifest: dict[str, Any], zmk_config_dir: Path) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("source_inventory", {}).get("define_entries", []):
        source_file = check["source_file"]
        expected = list(check["expected"])
        source_path = zmk_config_dir / source_file
        if not source_path.exists():
            results.append(
                Result(
                    f"zmk_source.define_entries.{source_file}",
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"missing define source file {source_file!r}",
                )
            )
            continue
        actual = define_entry_inventory(source_path.read_text(), check.get("prefix", ""))
        results.append(
            ordered_inventory_result(
                f"zmk_source.define_entries.{source_file}",
                "zmk_inventory",
                expected,
                actual,
            )
        )
    return results


def dts_alias_inventory(text: str, block_name: str) -> list[str]:
    block = top_level_text(extract_block(strip_c_style_comments(text), block_name))
    return [
        f"{match.group(1)}=&{match.group(2)}"
        for match in re.finditer(r"(?m)^\s*([A-Za-z0-9_-]+)\s*=\s*&([A-Za-z0-9_]+)\s*;", block)
    ]


def extract_dts_inventory_block(text: str, block_name: str) -> str:
    text = strip_c_style_comments(text)
    if block_name == "__top__":
        return text
    if block_name == "/":
        match = re.search(r"(?:^|[;\n])\s*/\s*\{", text)
        if not match:
            raise ValueError("root block '/' not found")
        start = match.end() - 1
        end = matching_block_end(text, start)
        return text[start + 1 : end]
    return extract_block(text, block_name)


def dts_node_inventory(text: str, block_name: str) -> list[str]:
    return [
        label or name
        for label, name, _ in dts_child_blocks(extract_dts_inventory_block(text, block_name))
    ]


def check_zmk_dts_node_inventory(manifest: dict[str, Any], zmk_config_dir: Path) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("source_inventory", {}).get("dts_node_inventories", []):
        source_file = check["source_file"]
        source_block = check["source_block"]
        expected = list(check["expected"])
        source_path = zmk_config_dir / source_file
        result_id = f"zmk_source.dts_nodes.{source_file}.{source_block}"
        if not source_path.exists():
            results.append(
                Result(
                    result_id,
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"missing DTS node source file {source_file!r}",
                )
            )
            continue
        try:
            actual = dts_node_inventory(source_path.read_text(), source_block)
        except ValueError as e:
            results.append(
                Result(
                    result_id,
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"invalid DTS node source {source_file!r}: {e}",
                )
            )
            continue
        results.append(ordered_inventory_result(result_id, "zmk_inventory", expected, actual))
    return results


def check_zmk_dts_alias_inventory(manifest: dict[str, Any], zmk_config_dir: Path) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("source_inventory", {}).get("dts_aliases", []):
        source_file = check["source_file"]
        expected = list(check["expected"])
        source_path = zmk_config_dir / source_file
        result_id = f"zmk_source.dts_aliases.{source_file}.{check['source_block']}"
        if not source_path.exists():
            results.append(
                Result(
                    result_id,
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"missing DTS alias source file {source_file!r}",
                )
            )
            continue
        try:
            actual = dts_alias_inventory(source_path.read_text(), check["source_block"])
        except ValueError as e:
            results.append(
                Result(
                    result_id,
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"invalid DTS alias source {source_file!r}: {e}",
                )
            )
            continue
        results.append(ordered_inventory_result(result_id, "zmk_inventory", expected, actual))
    return results


def physical_layout_attr_inventory(text: str, block_name: str) -> list[str]:
    block = extract_block(strip_c_style_comments(text), block_name)
    keys_body = extract_angle_property(block, "keys")
    return [
        ",".join(match.groups())
        for match in re.finditer(
            r"&key_physical_attrs\s+(-?\d+)\s+(-?\d+)\s+(-?\d+)\s+(-?\d+)\s+(-?\d+)\s+(-?\d+)\s+(-?\d+)",
            keys_body,
        )
    ]


def check_zmk_physical_layout_attr_inventory(
    manifest: dict[str, Any], zmk_config_dir: Path
) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("source_inventory", {}).get("physical_layout_attrs", []):
        source_file = check["source_file"]
        expected = list(check["expected"])
        source_path = zmk_config_dir / source_file
        if not source_path.exists():
            results.append(
                Result(
                    f"zmk_source.physical_layout_attrs.{source_file}",
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"missing physical layout source file {source_file!r}",
                )
            )
            continue
        try:
            actual = physical_layout_attr_inventory(source_path.read_text(), check["source_block"])
        except ValueError as e:
            results.append(
                Result(
                    f"zmk_source.physical_layout_attrs.{source_file}",
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"invalid physical layout source {source_file!r}: {e}",
                )
            )
            continue
        results.append(
            ordered_inventory_result(
                f"zmk_source.physical_layout_attrs.{source_file}",
                "zmk_inventory",
                expected,
                actual,
            )
        )
    return results


def input_behavior_binding_inventory(text: str, block_name: str) -> list[str]:
    block = extract_block(strip_c_style_comments(text), block_name)
    codes = extract_angle_property(block, "codes").split()
    bindings_body = extract_angle_property(block, "bindings")
    bindings = [
        f"&{match.group(1)} {match.group(2)}"
        for match in re.finditer(r"&([A-Za-z_][A-Za-z0-9_]*)\s+([A-Za-z_][A-Za-z0-9_]*)", bindings_body)
    ]
    if len(codes) != len(bindings):
        raise ValueError(
            f"codes/bindings length mismatch in {block_name!r}: {len(codes)} codes, {len(bindings)} bindings"
        )
    return [f"{code}:{binding}" for code, binding in zip(codes, bindings, strict=True)]


def check_zmk_input_behavior_binding_inventory(
    manifest: dict[str, Any], zmk_config_dir: Path
) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("source_inventory", {}).get("input_behavior_bindings", []):
        source_file = check["source_file"]
        expected = list(check["expected"])
        source_path = zmk_config_dir / source_file
        if not source_path.exists():
            results.append(
                Result(
                    f"zmk_source.input_behavior_bindings.{check['source_block']}",
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"missing input behavior source file {source_file!r}",
                )
            )
            continue
        try:
            actual = input_behavior_binding_inventory(source_path.read_text(), check["source_block"])
        except ValueError as e:
            results.append(
                Result(
                    f"zmk_source.input_behavior_bindings.{check['source_block']}",
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"invalid input behavior source {source_file!r}: {e}",
                )
            )
            continue
        results.append(
            ordered_inventory_result(
                f"zmk_source.input_behavior_bindings.{check['source_block']}",
                "zmk_inventory",
                expected,
                actual,
            )
        )
    return results


def input_listener_inventory(text: str, block_name: str) -> list[str]:
    block = extract_block(strip_c_style_comments(text), block_name)
    entries = [
        f"device={extract_ref_property(block, 'device')}",
    ]
    entries.extend(
        f"input-processors:{processor}"
        for processor in parse_angle_array_property(block, "input-processors")
    )

    try:
        lowspeed = extract_block(block, "lowspeedmode")
    except ValueError:
        return entries

    entries.append(f"lowspeed.layers={','.join(parse_angle_array_property(lowspeed, 'layers'))}")
    entries.extend(
        f"lowspeed.input-processors:{processor}"
        for processor in parse_angle_array_property(lowspeed, "input-processors")
    )
    return entries


def check_zmk_input_listener_inventory(
    manifest: dict[str, Any], zmk_config_dir: Path
) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("source_inventory", {}).get("input_listeners", []):
        source_file = check["source_file"]
        expected = list(check["expected"])
        source_path = zmk_config_dir / source_file
        result_id = f"zmk_source.input_listeners.{source_file}.{check['source_block']}"
        if not source_path.exists():
            results.append(
                Result(
                    result_id,
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"missing input listener source file {source_file!r}",
                )
            )
            continue
        try:
            actual = input_listener_inventory(source_path.read_text(), check["source_block"])
        except ValueError as e:
            results.append(
                Result(
                    result_id,
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"invalid input listener source {source_file!r}: {e}",
                )
            )
            continue
        results.append(
            ordered_inventory_result(
                result_id,
                "zmk_inventory",
                expected,
                actual,
            )
        )
    return results


def normalize_dts_property_value(value: str) -> str:
    value = value.strip()
    if value.startswith("<") and value.endswith(">"):
        return f"<{' '.join(value[1:-1].split())}>"
    if value.startswith('"') and value.endswith('"'):
        return value
    return " ".join(value.split())


def dts_property_inventory(text: str, block_name: str) -> list[str]:
    raw_block = extract_block(strip_c_style_comments(text), block_name)
    block = top_level_text(raw_block)
    entries: list[str] = []
    property_name = r"[#A-Za-z_][A-Za-z0-9_,#-]*"
    property_value = r"<.*?>|\".*?\"|[^;]+"
    pattern = re.compile(
        rf"(?<![A-Za-z0-9_,#-])(?P<name>{property_name})\s*"
        rf"(?:=\s*(?P<value>{property_value}))?\s*;",
        re.S,
    )
    for match in pattern.finditer(block):
        if "{" in raw_block[match.start() : match.end()]:
            continue
        name = match.group("name")
        value = match.group("value")
        if value is None:
            entries.append(name)
        else:
            entries.append(f"{name}={normalize_dts_property_value(value)}")
    return entries


def check_zmk_dts_property_inventory(manifest: dict[str, Any], zmk_config_dir: Path) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("source_inventory", {}).get("dts_properties", []):
        source_file = check["source_file"]
        source_block = check["source_block"]
        expected = list(check["expected"])
        source_path = zmk_config_dir / source_file
        result_id = f"zmk_source.dts_properties.{source_file}.{source_block}"
        if not source_path.exists():
            results.append(
                Result(
                    result_id,
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"missing DTS property source file {source_file!r}",
                )
            )
            continue
        try:
            actual = dts_property_inventory(source_path.read_text(), source_block)
        except ValueError as e:
            results.append(
                Result(
                    result_id,
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"invalid DTS property source {source_file!r}: {e}",
                )
            )
            continue
        results.append(
            ordered_inventory_result(
                result_id,
                "zmk_inventory",
                expected,
                actual,
            )
        )
    return results


def check_zmk_gpio_property_inventory(manifest: dict[str, Any], zmk_config_dir: Path) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("source_inventory", {}).get("gpio_properties", []):
        source_file = check["source_file"]
        expected = list(check["expected"])
        source_path = zmk_config_dir / source_file
        result_id = f"zmk_source.gpio_properties.{source_file}.{check['source_block']}.{check['source_property']}"
        if not source_path.exists():
            results.append(
                Result(
                    result_id,
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"missing GPIO property source file {source_file!r}",
                )
            )
            continue
        try:
            block = extract_block(source_path.read_text(), check["source_block"])
            actual = parse_gpio_property_with_flags(block, check["source_property"])
        except ValueError as e:
            results.append(
                Result(
                    result_id,
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"invalid GPIO property source {source_file!r}: {e}",
                )
            )
            continue
        results.append(ordered_inventory_result(result_id, "zmk_inventory", expected, actual))
    return results


def west_manifest_inventory(text: str) -> list[str]:
    items: list[str] = []
    section: str | None = None
    current: dict[str, str] | None = None

    def flush() -> None:
        nonlocal current
        if not current or section is None:
            current = None
            return
        if section == "remotes":
            items.append(f"remote:{current.get('name')}:url-base={current.get('url-base')}")
        elif section == "projects":
            item = (
                f"project:{current.get('name')}:remote={current.get('remote')}:"
                f"revision={current.get('revision')}"
            )
            if "import" in current:
                item += f":import={current['import']}"
            items.append(item)
        current = None

    for raw_line in text.splitlines():
        if match := re.fullmatch(r"\s{2}(remotes|projects|self):\s*", raw_line):
            flush()
            section = match.group(1)
            continue
        if re.fullmatch(r"\s{2}[A-Za-z0-9_-]+:\s*", raw_line):
            flush()
            section = None
            continue
        if section in {"remotes", "projects"}:
            if match := re.fullmatch(r"\s{4}-\s+name:\s+(.+?)\s*", raw_line):
                flush()
                current = {"name": match.group(1)}
                continue
            if current is not None and (
                match := re.fullmatch(r"\s{6}([A-Za-z0-9_-]+):\s+(.+?)\s*", raw_line)
            ):
                current[match.group(1)] = match.group(2)
                continue
        elif section == "self":
            if match := re.fullmatch(r"\s{4}path:\s+(.+?)\s*", raw_line):
                items.append(f"self:path={match.group(1)}")
    flush()
    return items


def check_west_manifest_inventory(manifest: dict[str, Any], zmk_config_dir: Path) -> list[Result]:
    expected = list(manifest.get("source_inventory", {}).get("west_manifest", []))
    source_path = zmk_config_dir / "west.yml"
    if not source_path.exists():
        return [
            Result(
                "zmk_source.west_manifest",
                "zmk_inventory",
                0,
                max(1, len(expected)),
                "missing west manifest 'west.yml'",
            )
        ]
    actual = west_manifest_inventory(source_path.read_text())
    return [ordered_inventory_result("zmk_source.west_manifest", "zmk_inventory", expected, actual)]


def yaml_scalar(value: str | None) -> str:
    if value is None:
        return ""
    value = value.strip()
    if len(value) >= 2 and value[0] == value[-1] and value[0] in {"'", '"'}:
        return value[1:-1]
    return value


def zmk_build_file_inventory(text: str) -> list[str]:
    items: list[str] = []
    current_include: dict[str, str] | None = None
    current_include_index = -1
    top_section: str | None = None
    build_section: str | None = None
    include_keys = {"board", "shield", "snippet"}

    def flush_include() -> None:
        nonlocal current_include
        if current_include:
            item = (
                f"include:board={current_include.get('board')}:"
                f"shield={current_include.get('shield')}"
            )
            if "snippet" in current_include:
                item += f":snippet={current_include['snippet']}"
            items.append(item)
        current_include = None

    for raw_line in text.splitlines():
        line = raw_line.split("#", 1)[0].rstrip()
        if not line.strip() or line.strip() == "---":
            continue
        indent = len(line) - len(line.lstrip(" "))

        if match := re.fullmatch(r"([A-Za-z0-9_-]+):(?:\s+(.+?))?", line):
            flush_include()
            top_section = match.group(1)
            build_section = None
            if top_section not in {"include", "build"}:
                items.append(f"unknown.top_level.{top_section}={yaml_scalar(match.group(2))}")
            continue

        if top_section == "include":
            if match := re.fullmatch(r"  -\s+([A-Za-z0-9_-]+):\s+(.+?)", line):
                flush_include()
                current_include_index += 1
                key = match.group(1)
                value = yaml_scalar(match.group(2))
                current_include = {}
                if key in include_keys:
                    current_include[key] = value
                else:
                    items.append(f"include.{current_include_index}.{key}={value}")
                continue
            if current_include is not None:
                if match := re.fullmatch(r"    ([A-Za-z0-9_-]+):\s+(.+?)", line):
                    key = match.group(1)
                    value = yaml_scalar(match.group(2))
                    if key in include_keys:
                        current_include[key] = value
                    else:
                        items.append(f"include.{current_include_index}.{key}={value}")
                    continue
            items.append(f"include.unparsed={line.strip()}")
            continue

        flush_include()
        if top_section == "build":
            if match := re.fullmatch(r"  ([A-Za-z0-9_-]+):(?:\s+(.+?))?", line):
                build_section = match.group(1)
                if build_section != "settings":
                    items.append(f"build.{build_section}={yaml_scalar(match.group(2))}")
                continue
            if build_section == "settings":
                if match := re.fullmatch(r"    ([A-Za-z0-9_-]+):\s+(.+?)", line):
                    key = match.group(1)
                    value = yaml_scalar(match.group(2))
                    if key == "board_root":
                        items.append(f"build.settings.board_root={value}")
                    else:
                        items.append(f"build.settings.{key}={value}")
                    continue
            if build_section is not None:
                if match := re.fullmatch(r"    ([A-Za-z0-9_-]+):\s+(.+?)", line):
                    items.append(
                        f"build.{build_section}.{match.group(1)}={yaml_scalar(match.group(2))}"
                    )
                    continue
            items.append(f"build.unparsed={line.strip()}")
            continue

        if top_section is not None:
            items.append(f"{top_section}.unparsed={line.strip()}")
        else:
            items.append(f"unknown.unparsed={line.strip()}")
    flush_include()
    return items


def check_zmk_build_file_inventory(
    manifest: dict[str, Any], zmk_config_dir: Path
) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("source_inventory", {}).get("build_files", []):
        source_file = check["source_file"]
        expected = list(check["expected"])
        source_path = zmk_config_dir / source_file
        result_id = f"zmk_source.build_file_inventory.{source_file}"
        if not source_path.exists():
            results.append(
                Result(
                    result_id,
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"missing ZMK build source file {source_file!r}",
                )
            )
            continue
        actual = zmk_build_file_inventory(source_path.read_text())
        results.append(ordered_inventory_result(result_id, "zmk_inventory", expected, actual))
    return results


def zmk_workflow_file_inventory(text: str) -> list[str]:
    items: list[str] = []
    top_section: str | None = None
    job_id: str | None = None

    for raw_line in text.splitlines():
        line = raw_line.split("#", 1)[0].rstrip()
        if not line.strip() or line.strip() == "---":
            continue

        if match := re.fullmatch(r"([A-Za-z0-9_-]+):(?:\s+(.+?))?", line):
            top_section = match.group(1)
            job_id = None
            value = yaml_scalar(match.group(2))
            if top_section == "name":
                items.append(f"workflow.name={value}")
            elif top_section == "on":
                if value.startswith("[") and value.endswith("]"):
                    triggers = ",".join(part.strip() for part in value[1:-1].split(","))
                    items.append(f"workflow.on={triggers}")
                else:
                    items.append(f"workflow.on={value}")
            elif top_section != "jobs":
                items.append(f"workflow.top_level.{top_section}={value}")
            continue

        if top_section == "jobs":
            if match := re.fullmatch(r"  ([A-Za-z0-9_-]+):(?:\s+(.+?))?", line):
                job_id = match.group(1)
                if match.group(2):
                    items.append(f"workflow.jobs.{job_id}={yaml_scalar(match.group(2))}")
                continue
            if job_id is not None:
                if match := re.fullmatch(r"    ([A-Za-z0-9_-]+):\s+(.+?)", line):
                    key = match.group(1)
                    value = yaml_scalar(match.group(2))
                    items.append(f"workflow.jobs.{job_id}.{key}={value}")
                    continue
            items.append(f"workflow.jobs.unparsed={line.strip()}")
            continue

        if top_section is not None:
            items.append(f"workflow.{top_section}.unparsed={line.strip()}")
        else:
            items.append(f"workflow.unparsed={line.strip()}")
    return items


def check_zmk_workflow_file_inventory(
    manifest: dict[str, Any], zmk_config_dir: Path
) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("source_inventory", {}).get("workflow_files", []):
        source_file = check["source_file"]
        expected = list(check["expected"])
        source_path = zmk_config_dir / source_file
        result_id = f"zmk_source.workflow_file_inventory.{source_file}"
        if not source_path.exists():
            results.append(
                Result(
                    result_id,
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"missing ZMK workflow source file {source_file!r}",
                )
            )
            continue
        actual = zmk_workflow_file_inventory(source_path.read_text())
        results.append(ordered_inventory_result(result_id, "zmk_inventory", expected, actual))
    return results


def compact_json_value(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def compact_json_scalar(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def zmk_json_file_inventory(value: dict[str, Any]) -> list[str]:
    items: list[str] = []
    known_top_level = {"id", "name", "layouts", "sensors"}

    if "id" in value:
        items.append(f"json.id={value['id']}")
    if "name" in value:
        items.append(f"json.name={value['name']}")

    layouts = value.get("layouts", {})
    if isinstance(layouts, dict):
        layout_names = sorted(layouts)
        items.append(f"json.layouts={','.join(layout_names)}")
        for layout_name in layout_names:
            layout = layouts[layout_name]
            if not isinstance(layout, dict):
                items.append(f"json.layouts.{layout_name}={compact_json_value(layout)}")
                continue
            if "name" in layout:
                items.append(f"json.layouts.{layout_name}.name={layout['name']}")
            if "layout" in layout:
                layout_value = layout["layout"]
                if isinstance(layout_value, list):
                    items.append(f"json.layouts.{layout_name}.layout_count={len(layout_value)}")
                else:
                    items.append(
                        f"json.layouts.{layout_name}.layout={compact_json_value(layout_value)}"
                    )
            for key in sorted(set(layout) - {"name", "layout"}):
                items.append(f"json.layouts.{layout_name}.{key}={compact_json_value(layout[key])}")
    elif "layouts" in value:
        items.append(f"json.layouts={compact_json_value(layouts)}")

    if "sensors" in value:
        items.append(f"json.sensors={compact_json_value(value['sensors'])}")

    for key in sorted(set(value) - known_top_level):
        items.append(f"json.top_level.{key}={compact_json_value(value[key])}")
    return items


def zmk_json_layout_entry_inventory(value: dict[str, Any], layout_name: str) -> list[str]:
    layouts = value.get("layouts", {})
    if not isinstance(layouts, dict):
        return [f"{layout_name}:layouts={compact_json_value(layouts)}"]

    layout = layouts.get(layout_name)
    if not isinstance(layout, dict):
        return [f"{layout_name}:layout={compact_json_value(layout)}"]

    entries = layout.get("layout", [])
    if not isinstance(entries, list):
        return [f"{layout_name}:layout={compact_json_value(entries)}"]

    items: list[str] = []
    preferred_keys = ["row", "col", "x", "y", "u", "h", "w"]
    preferred_key_set = set(preferred_keys)
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            items.append(f"{layout_name}[{index}]={compact_json_value(entry)}")
            continue
        parts = [
            f"{key}={compact_json_scalar(entry[key])}" for key in preferred_keys if key in entry
        ]
        parts.extend(
            f"{key}={compact_json_value(entry[key])}"
            for key in sorted(set(entry) - preferred_key_set)
        )
        items.append(f"{layout_name}[{index}]={','.join(parts)}")
    return items


def check_zmk_json_file_inventory(manifest: dict[str, Any], zmk_config_dir: Path) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("source_inventory", {}).get("json_files", []):
        source_file = check["source_file"]
        expected = list(check["expected"])
        source_path = zmk_config_dir / source_file
        result_id = f"zmk_source.json_file_inventory.{source_file}"
        if not source_path.exists():
            results.append(
                Result(
                    result_id,
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"missing ZMK JSON source file {source_file!r}",
                )
            )
            continue
        try:
            source_json = load_json(source_path)
        except json.JSONDecodeError as e:
            results.append(
                Result(
                    result_id,
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"invalid ZMK JSON source file {source_file!r}: {e}",
                )
            )
            continue
        if not isinstance(source_json, dict):
            results.append(
                Result(
                    result_id,
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"ZMK JSON source file {source_file!r} must contain an object at the root",
                )
            )
            continue
        actual = zmk_json_file_inventory(source_json)
        results.append(ordered_inventory_result(result_id, "zmk_inventory", expected, actual))
    return results


def check_zmk_json_layout_entry_inventory(
    manifest: dict[str, Any], zmk_config_dir: Path
) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("source_inventory", {}).get("json_layout_entries", []):
        source_file = check["source_file"]
        layout_name = check["layout_name"]
        expected = list(check["expected"])
        source_path = zmk_config_dir / source_file
        result_id = f"zmk_source.json_layout_entries.{source_file}.{layout_name}"
        if not source_path.exists():
            results.append(
                Result(
                    result_id,
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"missing ZMK JSON source file {source_file!r}",
                )
            )
            continue
        try:
            source_json = load_json(source_path)
        except json.JSONDecodeError as e:
            results.append(
                Result(
                    result_id,
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"invalid ZMK JSON source file {source_file!r}: {e}",
                )
            )
            continue
        if not isinstance(source_json, dict):
            results.append(
                Result(
                    result_id,
                    "zmk_inventory",
                    0,
                    max(1, len(expected)),
                    f"ZMK JSON source file {source_file!r} must contain an object at the root",
                )
            )
            continue
        actual = zmk_json_layout_entry_inventory(source_json, layout_name)
        results.append(ordered_inventory_result(result_id, "zmk_inventory", expected, actual))
    return results


def check_zmk_dts_status_inventory(manifest: dict[str, Any], zmk_config_dir: Path) -> list[Result]:
    inventory = manifest.get("source_inventory", {})
    results: list[Result] = []
    for check in inventory.get("dts_status_files", []):
        source_file = check["source_file"]
        expected = list(check["expected"])
        actual_list = [
            f"{node}:{status}"
            for node, status in dts_status_nodes((zmk_config_dir / source_file).read_text())
        ]
        actual = set(actual_list)
        expected_set = set(expected)
        missing = sorted(expected_set - actual)
        extra = sorted(actual - expected_set)
        duplicates = sorted(item for item in actual if actual_list.count(item) > 1)
        passed = len(expected_set) - len(missing)
        total = len(expected_set) + len(extra) + len(duplicates)
        messages: list[str] = []
        if missing:
            messages.append(f"missing status nodes {missing!r}")
        if extra:
            messages.append(f"unclassified status nodes {extra!r}")
        if duplicates:
            messages.append(f"duplicated status nodes {duplicates!r}")
        results.append(
            Result(
                f"dts_status_inventory.{source_file}",
                "zmk_inventory",
                passed,
                total,
                "ok" if not messages else "; ".join(messages),
            )
        )
    return results


def check_zmk_pin_values(
    manifest: dict[str, Any],
    keyboard: dict[str, Any],
    zmk_config_dir: Path,
) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("zmk_pin_values", []):
        source_path = zmk_config_dir / check["source_file"]
        block = extract_block(source_path.read_text(), check["source_block"])
        actual_source = parse_gpio_property(block, check["source_property"])
        expected_source = list(check["expected"])
        source_ok = actual_source == expected_source
        passed = 1 if source_ok else 0
        total = 1
        messages = [f"source expected {expected_source!r}, got {actual_source!r}"]

        for target_path in check.get("target_paths", []):
            try:
                actual_target = path_get(keyboard, target_path)
            except KeyError:
                actual_target = None
            expected_target: Any = expected_source
            if len(expected_source) == 1 and not isinstance(actual_target, list):
                expected_target = expected_source[0]
            target_ok = actual_target == expected_target
            passed += 1 if target_ok else 0
            total += 1
            messages.append(f"{target_path} expected {expected_target!r}, got {actual_target!r}")

        results.append(
            Result(
                check["id"],
                "zmk_pin",
                passed,
                total,
                "ok" if passed == total else "; ".join(messages),
            )
        )
    return results


def check_rust_const_values(manifest: dict[str, Any], zmk_config_dir: Path, project_root: Path) -> list[Result]:
    results: list[Result] = []
    source_cache: dict[Path, dict[str, str]] = {}
    for check in manifest.get("rust_const_values", []):
        passed = 0
        total = 0
        messages: list[str] = []

        expected = check["expected"]
        if "source_file" in check and "source_key" in check:
            source_path = zmk_config_dir / check["source_file"]
            total += 1
            if not source_path.exists():
                messages.append(f"missing source Kconfig file {check['source_file']!r}")
            else:
                if source_path not in source_cache:
                    source_cache[source_path] = parse_kconfig(source_path)
                actual_source = source_cache[source_path].get(check["source_key"])
                expected_source = str(expected).lower() if isinstance(expected, bool) else str(expected)
                if actual_source == expected_source:
                    passed += 1
                else:
                    messages.append(f"source expected {expected_source!r}, got {actual_source!r}")

        total += 1
        try:
            actual_const = parse_rust_const(project_root / check["target_file"], check["target_const"])
        except (OSError, ValueError) as e:
            messages.append(str(e))
        else:
            if actual_const == expected:
                passed += 1
            else:
                messages.append(f"{check['target_const']} expected {expected!r}, got {actual_const!r}")

        results.append(
            Result(
                check["id"],
                "rust_const",
                passed,
                total,
                "ok" if not messages else "; ".join(messages),
            )
        )
    return results


def check_rust_byte_arrays(manifest: dict[str, Any], project_root: Path) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("rust_byte_arrays", []):
        passed = 0
        total = 2
        messages: list[str] = []
        try:
            values = parse_rust_byte_array(project_root / check["target_file"], check["target_const"])
        except (OSError, ValueError) as e:
            messages.append(str(e))
        else:
            expected_len = int(check["expected_len"])
            if len(values) == expected_len:
                passed += 1
            else:
                messages.append(f"length expected {expected_len}, got {len(values)}")
            actual_sha256 = hashlib.sha256(bytes(values)).hexdigest()
            expected_sha256 = check["expected_sha256"]
            if actual_sha256 == expected_sha256:
                passed += 1
            else:
                messages.append(f"sha256 expected {expected_sha256}, got {actual_sha256}")
        results.append(
            Result(
                check["id"],
                "rust_byte_array",
                passed,
                total,
                "ok" if not messages else "; ".join(messages),
            )
        )
    return results


def check_code_contains(manifest: dict[str, Any], project_root: Path) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("code_contains", []):
        text = (project_root / check["file"]).read_text()
        needles = list(check["needles"])
        passed = sum(1 for needle in needles if needle in text)
        missing = [needle for needle in needles if needle not in text]
        results.append(
            Result(
                check["id"],
                "code",
                passed,
                len(needles),
                "ok" if not missing else f"missing {missing!r}",
            )
        )
    return results


def collect_vial_positions(value: Any) -> list[tuple[int, int]]:
    positions: list[tuple[int, int]] = []
    if isinstance(value, str):
        if match := re.fullmatch(r"(\d+),(\d+)", value):
            positions.append((int(match.group(1)), int(match.group(2))))
    elif isinstance(value, list):
        for item in value:
            positions.extend(collect_vial_positions(item))
    elif isinstance(value, dict):
        for item in value.values():
            positions.extend(collect_vial_positions(item))
    return positions


def zmk_json_positions(value: dict[str, Any]) -> list[tuple[int, int]]:
    layout = value["layouts"]["default_layout"]["layout"]
    return [(int(item["row"]), int(item["col"])) for item in layout]


def zmk_matrix_transform(path: Path) -> tuple[int, int, list[tuple[int, int]]]:
    block = extract_block(strip_c_style_comments(path.read_text()), "default_transform")
    rows = angle_int_property(block, "rows")
    cols = angle_int_property(block, "columns")
    map_body = strip_c_style_comments(extract_angle_property(block, "map"))
    positions = [
        (int(match.group(1)), int(match.group(2)))
        for match in re.finditer(r"RC\(\s*(\d+)\s*,\s*(\d+)\s*\)", map_body)
    ]
    return rows, cols, positions


def check_zmk_physical_layout_chain(zmk_config_dir: Path) -> list[Result]:
    dtsi_text = strip_c_style_comments(
        (zmk_config_dir / "boards/shields/lalapadgen2/lalapadgen2.dtsi").read_text()
    )
    layout_text = strip_c_style_comments(
        (zmk_config_dir / "boards/shields/lalapadgen2/lalapadgen2-layouts.dtsi").read_text()
    )
    rows, cols, transform_positions = zmk_matrix_transform(
        zmk_config_dir / "boards/shields/lalapadgen2/lalapadgen2.dtsi"
    )

    chosen_block = extract_block(dtsi_text, "chosen")
    layout_block = extract_block(layout_text, "lalapadgen2_physical_layout")
    checks = [
        (
            "chosen.zmk,physical-layout",
            extract_ref_property(chosen_block, "zmk,physical-layout"),
            "&lalapadgen2_physical_layout",
        ),
        ("physical_layout.kscan", extract_ref_property(layout_block, "kscan"), "&kscan0"),
        (
            "physical_layout.transform",
            extract_ref_property(layout_block, "transform"),
            "&default_transform",
        ),
    ]
    passed = sum(1 for _, actual, expected in checks if actual == expected)
    messages = [
        f"{name} expected {expected!r}, got {actual!r}"
        for name, actual, expected in checks
        if actual != expected
    ]
    results = [
        Result(
            "zmk_physical_layout_chain",
            "zmk_transform",
            passed,
            len(checks),
            "ok" if not messages else "; ".join(messages),
        )
    ]

    keys_body = extract_angle_property(layout_block, "keys")
    key_count = len(re.findall(r"&key_physical_attrs\b", keys_body))
    expected_key_count = len(transform_positions)
    unique_transform_positions = set(transform_positions)
    in_bounds_positions = {
        (row, col)
        for row in range(rows)
        for col in range(cols)
        if (row, col) in unique_transform_positions
    }
    ok = key_count == expected_key_count == len(in_bounds_positions)
    results.append(
        Result(
            "zmk_physical_layout_key_count",
            "zmk_transform",
            1 if ok else 0,
            1,
            "ok"
            if ok
            else f"physical layout keys {key_count}, transform positions {expected_key_count}",
        )
    )
    return results


def check_zmk_matrix_transform(manifest: dict[str, Any], zmk_config_dir: Path) -> list[Result]:
    transform_path = zmk_config_dir / "boards/shields/lalapadgen2/lalapadgen2.dtsi"
    zmk_layout = load_json(zmk_config_dir / "lalapadgen2.json")
    rows, cols, transform_positions = zmk_matrix_transform(transform_path)
    expected_layout = manifest["layout"]
    expected_positions = zmk_json_positions(zmk_layout)
    results: list[Result] = []

    shape_checks = [
        ("rows", rows, expected_layout["rows"]),
        ("cols", cols, expected_layout["cols"]),
    ]
    shape_passed = sum(1 for _, actual, expected in shape_checks if actual == expected)
    shape_messages = [
        f"{name} expected {expected!r}, got {actual!r}"
        for name, actual, expected in shape_checks
        if actual != expected
    ]
    results.append(
        Result(
            "zmk_matrix_transform_shape",
            "zmk_transform",
            shape_passed,
            len(shape_checks),
            "ok" if not shape_messages else "; ".join(shape_messages),
        )
    )

    position_total = max(len(expected_positions), len(transform_positions))
    position_passed = 0
    mismatches: list[str] = []
    for index in range(position_total):
        expected = expected_positions[index] if index < len(expected_positions) else None
        actual = transform_positions[index] if index < len(transform_positions) else None
        if actual == expected:
            position_passed += 1
        else:
            mismatches.append(f"p{index}: expected {expected!r}, got {actual!r}")
    results.append(
        Result(
            "zmk_matrix_transform_matches_layout_json",
            "zmk_transform",
            position_passed,
            position_total,
            "ok" if not mismatches else "; ".join(mismatches[:8]),
        )
    )
    results.extend(check_zmk_physical_layout_chain(zmk_config_dir))
    return results


def check_vial_layout(manifest: dict[str, Any], project_root: Path, zmk_config_dir: Path) -> list[Result]:
    vial = load_json(project_root / "vial.json")
    zmk_layout = load_json(zmk_config_dir / "lalapadgen2.json")
    results: list[Result] = []

    expected_layout = manifest["layout"]
    matrix_checks = [
        ("rows", vial["matrix"].get("rows"), expected_layout["rows"]),
        ("cols", vial["matrix"].get("cols"), expected_layout["cols"]),
    ]
    passed = sum(1 for _, actual, expected in matrix_checks if actual == expected)
    messages = [
        f"{name} expected {expected!r}, got {actual!r}"
        for name, actual, expected in matrix_checks
        if actual != expected
    ]
    results.append(
        Result(
            "vial_matrix_shape",
            "vial",
            passed,
            len(matrix_checks),
            "ok" if not messages else "; ".join(messages),
        )
    )

    expected_positions = zmk_json_positions(zmk_layout)
    actual_positions = collect_vial_positions(vial["layouts"]["keymap"])
    position_total = max(len(expected_positions), len(actual_positions))
    position_passed = 0
    mismatches: list[str] = []
    for index in range(position_total):
        expected = expected_positions[index] if index < len(expected_positions) else None
        actual = actual_positions[index] if index < len(actual_positions) else None
        if actual == expected:
            position_passed += 1
        else:
            mismatches.append(f"p{index}: expected {expected!r}, got {actual!r}")
    results.append(
        Result(
            "vial_positions_match_zmk_layout_json",
            "vial",
            position_passed,
            position_total,
            "ok" if not mismatches else "; ".join(mismatches[:8]),
        )
    )

    expected_names = list(
        manifest.get(
            "vial_custom_keycodes",
            manifest.get("layout", {}).get("vial_custom_keycodes", []),
        )
    )
    actual_names = [item.get("name") for item in vial.get("customKeycodes", [])]
    custom_total = max(len(expected_names), len(actual_names))
    custom_passed = 0
    custom_mismatches: list[str] = []
    for index in range(custom_total):
        expected = expected_names[index] if index < len(expected_names) else None
        actual = actual_names[index] if index < len(actual_names) else None
        if actual == expected:
            custom_passed += 1
        else:
            custom_mismatches.append(f"c{index}: expected {expected!r}, got {actual!r}")
    results.append(
        Result(
            "vial_custom_keycodes_match_user_key_order",
            "vial",
            custom_passed,
            custom_total,
            "ok" if not custom_mismatches else "; ".join(custom_mismatches),
        )
    )
    return results


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


def zmk_behavior_source_values(source_text: str) -> dict[str, Any]:
    mt_block = extract_block(source_text, "&mt")
    lt_block = extract_block(source_text, "&lt")
    mt2_block = extract_block(source_text, "mt2")
    return {
        "zmk_mt_quick_tap_ms": angle_int_property(mt_block, "quick-tap-ms"),
        "zmk_lt_quick_tap_ms": angle_int_property(lt_block, "quick-tap-ms"),
        "zmk_mt2_flavor": scalar_property(mt2_block, "flavor"),
        "zmk_mt2_tapping_term_ms": angle_int_property(mt2_block, "tapping-term-ms"),
        "zmk_mt2_quick_tap_ms": angle_int_property(mt2_block, "quick-tap-ms"),
        "zmk_mt2_require_prior_idle_ms": angle_int_property(mt2_block, "require-prior-idle-ms"),
    }


def check_zmk_behavior_source(manifest: dict[str, Any], source_text: str) -> list[Result]:
    results: list[Result] = []
    source_values = zmk_behavior_source_values(source_text)
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


def transform_zmk_behavior_value(value: Any, transform: str | None) -> Any:
    if transform is None:
        return value
    if transform == "ms_string":
        return f"{value}ms"
    raise ValueError(f"unknown ZMK behavior mirror transform {transform!r}")


def check_zmk_behavior_mirrors(
    manifest: dict[str, Any],
    keyboard: dict[str, Any],
    source_text: str,
) -> list[Result]:
    results: list[Result] = []
    source_values = zmk_behavior_source_values(source_text)
    for check in manifest.get("zmk_behavior_mirrors", []):
        source_id = check["source_id"]
        transform = check.get("transform")
        passed = 0
        total = 2 if "source_expected" in check else 1
        messages: list[str] = []
        source_actual = source_values.get(source_id)
        if "source_expected" in check:
            source_expected = check["source_expected"]
            if source_actual == source_expected:
                passed += 1
            else:
                messages.append(
                    f"source {source_id} expected {source_expected!r}, got {source_actual!r}"
                )
        try:
            expected_target = transform_zmk_behavior_value(source_actual, transform)
            target_actual = path_get(keyboard, check["target_path"])
        except (KeyError, ValueError) as e:
            messages.append(str(e))
        else:
            if target_actual == expected_target:
                passed += 1
            else:
                messages.append(
                    f"target {check['target_path']} expected {expected_target!r}, got {target_actual!r}"
                )
        results.append(
            Result(
                check["id"],
                "zmk_behavior_mirror",
                passed,
                total,
                "ok" if not messages else "; ".join(messages),
            )
        )
    return results


def check_zmk_source(
    manifest: dict[str, Any],
    keyboard: dict[str, Any],
    project_root: Path,
    zmk_keymap_path: Path,
    required: bool,
) -> list[Result]:
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

    zmk_config_dir = zmk_keymap_path.parent
    source_text = zmk_keymap_path.read_text()
    results.extend(check_zmk_keymap_layer_inventory(manifest, source_text))
    results.extend(check_zmk_behavior_inventory(manifest, source_text))
    raw_source_layers = raw_zmk_keymap_rows(zmk_keymap_path)
    results.extend(check_zmk_source_deltas(manifest, raw_source_layers))
    source_layers = apply_documented_rmk_deltas(manifest, raw_source_layers)
    results.extend(check_zmk_source_scenarios(manifest, keyboard, source_layers))
    results.extend(check_zmk_source_layer_resolution(keyboard, source_layers))
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
    allowed_extra_combos = source_combo_delta_set_from_manifest(manifest)
    for extra in sorted(allowed_extra_combos):
        ok = extra in manifest_combos
        source_message = (
            "source already includes combo"
            if extra in expected_combos
            else "documented RMK combo delta"
        )
        results.append(
            Result(
                f"zmk_source.combo_delta.{combo_id(extra)}",
                "zmk_source_combo_delta",
                1 if ok else 0,
                1,
                source_message if ok else f"manifest missing documented combo delta {extra!r}",
            )
        )

    extra_combos = manifest_combos - expected_combos - allowed_extra_combos
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
    results.extend(check_zmk_behavior_mirrors(manifest, keyboard, source_text))
    results.extend(check_zmk_config_values(manifest, keyboard, zmk_config_dir))
    results.extend(check_zmk_config_mirrors(manifest, zmk_config_dir))
    results.extend(check_zmk_source_file_inventory(manifest, zmk_config_dir))
    results.extend(check_zmk_repo_file_inventory(manifest, zmk_config_dir))
    results.extend(check_zmk_include_inventory(manifest, zmk_config_dir))
    results.extend(check_zmk_kconfig_entry_inventory(manifest, zmk_config_dir))
    results.extend(check_zmk_kconfig_line_inventory(manifest, zmk_config_dir))
    results.extend(check_zmk_disabled_kconfig_line_inventory(manifest, zmk_config_dir))
    results.extend(check_zmk_dts_node_inventory(manifest, zmk_config_dir))
    results.extend(check_zmk_dts_alias_inventory(manifest, zmk_config_dir))
    results.extend(check_zmk_behavior_property_inventory(manifest, zmk_config_dir))
    results.extend(check_zmk_combo_property_inventory(manifest, zmk_config_dir))
    results.extend(check_zmk_define_entry_inventory(manifest, zmk_config_dir))
    results.extend(check_zmk_physical_layout_attr_inventory(manifest, zmk_config_dir))
    results.extend(check_zmk_input_behavior_binding_inventory(manifest, zmk_config_dir))
    results.extend(check_zmk_input_listener_inventory(manifest, zmk_config_dir))
    results.extend(check_zmk_dts_property_inventory(manifest, zmk_config_dir))
    results.extend(check_zmk_gpio_property_inventory(manifest, zmk_config_dir))
    results.extend(check_west_manifest_inventory(manifest, zmk_config_dir))
    results.extend(check_zmk_build_file_inventory(manifest, zmk_config_dir))
    results.extend(check_zmk_workflow_file_inventory(manifest, zmk_config_dir))
    results.extend(check_zmk_json_file_inventory(manifest, zmk_config_dir))
    results.extend(check_zmk_json_layout_entry_inventory(manifest, zmk_config_dir))
    results.extend(check_zmk_config_inventory(manifest, zmk_config_dir))
    results.extend(check_zmk_dts_status_inventory(manifest, zmk_config_dir))
    results.extend(check_zmk_pin_values(manifest, keyboard, zmk_config_dir))
    results.extend(check_source_regex_values(manifest, zmk_config_dir))
    results.extend(check_zmk_matrix_transform(manifest, zmk_config_dir))
    results.extend(check_rust_const_values(manifest, zmk_config_dir, project_root))
    results.extend(check_rust_byte_arrays(manifest, project_root))
    results.extend(check_vial_layout(manifest, project_root, zmk_config_dir))
    return results


def combo_set_from_manifest(manifest: dict[str, Any]) -> set[tuple[tuple[str, ...], str, int]]:
    return {
        (tuple(combo["actions"]), combo["output"], int(combo["layer"]))
        for combo in manifest.get("combos", [])
    }


def source_combo_delta_set_from_manifest(
    manifest: dict[str, Any],
) -> set[tuple[tuple[str, ...], str, int]]:
    return {
        (tuple(combo["actions"]), combo["output"], int(combo["layer"]))
        for combo in manifest.get("source_combo_deltas", [])
    }


def combo_id(combo: tuple[tuple[str, ...], str, int]) -> str:
    actions, output, layer = combo
    raw = "_".join([*actions, output, f"layer{layer}"]).lower()
    return re.sub(r"[^a-z0-9]+", "_", raw).strip("_")


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
    project_root = keyboard_path.resolve().parent
    manifest = load_toml(manifest_path)
    keyboard = load_toml(keyboard_path)
    results: list[Result] = []
    results.extend(check_layout(manifest, keyboard))
    results.extend(check_keymap_shape(manifest, keyboard))
    results.extend(check_keymap_rows(manifest, keyboard))
    results.extend(check_config_values(manifest, keyboard))
    results.extend(check_behavior_values(manifest, keyboard))
    results.extend(check_combos(manifest, keyboard))
    results.extend(check_scenarios(manifest, keyboard))
    results.extend(check_code_contains(manifest, project_root))
    results.extend(
        check_zmk_source(
            manifest,
            keyboard,
            project_root,
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
