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
IMPLEMENTED_PORTING_STATUSES = frozenset(
    {"ported", "ported_by_behavior", "ported_by_config_image"}
)
DOCUMENTED_GAP_PORTING_STATUSES = frozenset({"not_ported"})
VALID_PORTING_STATUSES = IMPLEMENTED_PORTING_STATUSES | DOCUMENTED_GAP_PORTING_STATUSES

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


@dataclass
class PortingStatusSummary:
    total: int
    implemented: int
    rate: float | None
    by_status: dict[str, int]
    remaining: list[dict[str, str]]


@dataclass
class CoverageBucket:
    passed: int
    total: int
    rate: float | None


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


def path_list_get(root: dict[str, Any], path: list[str]) -> Any:
    value: Any = root
    for part in path:
        if not isinstance(value, dict) or part not in value:
            raise KeyError(".".join(path))
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


def hold_action_layer(action: str) -> int | None:
    if action.startswith("LT("):
        inside = action.removeprefix("LT(").removesuffix(")")
        pieces = [piece.strip() for piece in inside.split(",")]
        if pieces and pieces[0].isdigit():
            return int(pieces[0])
    if action.startswith("MO("):
        inside = action.removeprefix("MO(").removesuffix(")").strip()
        if inside.isdigit():
            return int(inside)
    return None


def layer_tap_parts(action: str) -> tuple[int | None, str | None, str | None]:
    if not action.startswith("LT("):
        return None, None, None
    inside = action.removeprefix("LT(").removesuffix(")")
    pieces = [piece.strip() for piece in inside.split(",")]
    layer = int(pieces[0]) if pieces and pieces[0].isdigit() else None
    tap = pieces[1] if len(pieces) >= 2 else None
    profile = pieces[2] if len(pieces) >= 3 else None
    return layer, tap, profile


def check_hold_declared_layer(
    hold: dict[str, Any],
    action: str,
    expected_action: str,
    messages: list[str],
    prefix: str = "",
) -> bool:
    declared_layer = int(hold["activates_layer"])
    expected_layer = hold_action_layer(expected_action)
    actual_layer = hold_action_layer(action)
    if expected_layer != declared_layer:
        messages.append(
            f"{prefix}hold declared layer expected {expected_layer!r} from "
            f"{expected_action!r}, got activates_layer {declared_layer}"
        )
        return False
    if actual_layer != declared_layer:
        messages.append(
            f"{prefix}hold action layer expected {declared_layer}, got {actual_layer!r} from {action!r}"
        )
        return False
    return True


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


def split_rect_positions(rect: dict[str, Any]) -> set[tuple[int, int]]:
    row_offset = int(rect["row_offset"])
    col_offset = int(rect["col_offset"])
    rows = int(rect["rows"])
    cols = int(rect["cols"])
    return {
        (row, col)
        for row in range(row_offset, row_offset + rows)
        for col in range(col_offset, col_offset + cols)
    }


def split_rect_checks(prefix: str, actual: dict[str, Any], expected: dict[str, Any]) -> tuple[int, int, list[str]]:
    checks = [
        ("row_offset", actual.get("row_offset"), expected.get("row_offset")),
        ("col_offset", actual.get("col_offset"), expected.get("col_offset")),
        ("rows", actual.get("rows"), expected.get("rows")),
        ("cols", actual.get("cols"), expected.get("cols")),
        ("matrix_type", actual.get("matrix", {}).get("matrix_type"), expected.get("matrix_type")),
        ("row_pins", len(actual.get("matrix", {}).get("row_pins", [])), expected.get("rows")),
        ("col_pins", len(actual.get("matrix", {}).get("col_pins", [])), expected.get("cols")),
    ]
    if "row_pins" in expected:
        checks.append(("row_pins_order", actual.get("matrix", {}).get("row_pins", []), expected["row_pins"]))
    if "col_pins" in expected:
        checks.append(("col_pins_order", actual.get("matrix", {}).get("col_pins", []), expected["col_pins"]))
    passed = sum(1 for _, actual_value, expected_value in checks if actual_value == expected_value)
    messages = [
        f"{prefix}.{name} expected {expected_value!r}, got {actual_value!r}"
        for name, actual_value, expected_value in checks
        if actual_value != expected_value
    ]
    return passed, len(checks), messages


def check_split_footprint(manifest: dict[str, Any], config: dict[str, Any]) -> list[Result]:
    expected = manifest.get("split_footprint", {})
    if not expected:
        return []

    split = config.get("split", {})
    rmk = config.get("rmk", {})
    expected_peripherals = list(expected.get("peripherals", []))
    actual_peripherals = list(split.get("peripheral", []))
    results: list[Result] = []

    connection_checks = [
        ("split.connection", split.get("connection"), expected.get("connection")),
        ("peripheral count", len(actual_peripherals), len(expected_peripherals)),
        ("rmk.split_peripherals_num", rmk.get("split_peripherals_num"), len(expected_peripherals)),
        ("top-level matrix absent", "matrix" not in config, True),
        ("split.central.serial absent", "serial" not in split.get("central", {}), True),
    ]
    connection_checks.extend(
        (
            f"split.peripheral.{index}.serial absent",
            "serial" not in peripheral,
            True,
        )
        for index, peripheral in enumerate(actual_peripherals)
    )
    connection_passed = sum(1 for _, actual, want in connection_checks if actual == want)
    connection_messages = [
        f"{name} expected {want!r}, got {actual!r}"
        for name, actual, want in connection_checks
        if actual != want
    ]
    results.append(
        Result(
            "split_connection_and_count",
            "split",
            connection_passed,
            len(connection_checks),
            "ok" if not connection_messages else "; ".join(connection_messages),
        )
    )

    central_passed, central_total, central_messages = split_rect_checks(
        "central", split.get("central", {}), expected.get("central", {})
    )
    results.append(
        Result(
            "split_central_matrix_footprint",
            "split",
            central_passed,
            central_total,
            "ok" if not central_messages else "; ".join(central_messages),
        )
    )

    for index, expected_peripheral in enumerate(expected_peripherals):
        actual_peripheral = actual_peripherals[index] if index < len(actual_peripherals) else {}
        passed, total, messages = split_rect_checks(
            f"peripheral{index}", actual_peripheral, expected_peripheral
        )
        results.append(
            Result(
                f"split_peripheral{index}_matrix_footprint",
                "split",
                passed,
                total,
                "ok" if not messages else "; ".join(messages),
            )
        )

    actual_scan_parts = [split.get("central", {})] + actual_peripherals
    actual_scanned_positions: set[tuple[int, int]] = set()
    overlapping_positions: set[tuple[int, int]] = set()
    for part in actual_scan_parts:
        if not all(field in part for field in ("row_offset", "col_offset", "rows", "cols")):
            continue
        positions = split_rect_positions(part)
        overlapping_positions |= actual_scanned_positions & positions
        actual_scanned_positions |= positions

    expected_scanned_positions = set()
    for part in [expected.get("central", {})] + expected_peripherals:
        expected_scanned_positions |= split_rect_positions(part)

    expected_rows = int(config["layout"]["rows"])
    expected_cols = int(config["layout"]["cols"])
    out_of_bounds = sorted(
        (row, col)
        for row, col in actual_scanned_positions
        if not (0 <= row < expected_rows and 0 <= col < expected_cols)
    )
    missing_scanned = sorted(expected_scanned_positions - actual_scanned_positions)
    extra_scanned = sorted(actual_scanned_positions - expected_scanned_positions)
    scanned_total = len(expected_scanned_positions | actual_scanned_positions) + len(overlapping_positions)
    scanned_passed = len(expected_scanned_positions & actual_scanned_positions)
    messages: list[str] = []
    if missing_scanned:
        messages.append(f"missing scan positions {missing_scanned[:8]!r}")
    if extra_scanned:
        messages.append(f"unexpected scan positions {extra_scanned[:8]!r}")
    if overlapping_positions:
        messages.append(f"overlapping scan positions {sorted(overlapping_positions)[:8]!r}")
    if out_of_bounds:
        messages.append(f"out-of-bounds scan positions {out_of_bounds[:8]!r}")
    results.append(
        Result(
            "split_scanned_positions_match_expected_footprint",
            "split",
            scanned_passed,
            scanned_total,
            "ok" if not messages else "; ".join(messages),
        )
    )

    virtual_rows = {int(row) for row in expected.get("virtual_rows", [])}
    action_positions = set(keyboard_toml_vial_positions(config))
    virtual_action_positions = action_positions - actual_scanned_positions
    unexpected_virtual_actions = sorted(
        position for position in virtual_action_positions if position[0] not in virtual_rows
    )
    virtual_total = len(virtual_action_positions)
    virtual_passed = virtual_total - len(unexpected_virtual_actions)
    results.append(
        Result(
            "split_non_scanned_actions_are_virtual_rows",
            "split",
            virtual_passed,
            virtual_total,
            "ok"
            if not unexpected_virtual_actions
            else f"non-scanned action positions outside virtual rows {unexpected_virtual_actions[:8]!r}",
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
        total = 1 + len(holds) * 2
        messages: list[str] = []
        for hold in holds:
            action = km[0][int(hold["row"])][int(hold["col"])]
            expected_action = hold["expected_action"]
            if action == expected_action:
                passed += 1
            else:
                messages.append(f"hold action expected {expected_action!r}, got {action!r}")
            if check_hold_declared_layer(hold, action, expected_action, messages):
                passed += 1

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
        total = 1 + len(holds) * 2
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
            if check_hold_declared_layer(hold, action, expected_action, messages, "source "):
                passed += 1

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


def gpio_entry_pin_and_flags(entry: str) -> tuple[str, set[str]]:
    controller, pin_text, flags_text = entry.split(":", 2)
    return zmk_pin_to_rmk(controller, int(pin_text)), set(flags_text.split("|"))


def rust_text_contains_all(project_root: Path, file_name: str, needles: list[str]) -> tuple[int, list[str]]:
    text = (project_root / file_name).read_text()
    missing = [needle for needle in needles if needle not in text]
    return len(needles) - len(missing), missing


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
    declared_len: int | None
    if match:
        declared_len = int(match.group("len"))
    else:
        match = re.search(
            rf"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?const\s+{re.escape(name)}\s*:\s*&\[u8\]\s*=\s*&\[(?P<body>.*?)\];",
            text,
            re.S,
        )
        declared_len = None
    if not match:
        raise ValueError(f"byte array const {name!r} not found in {path}")
    values = [
        int(value, 16) if value.lower().startswith("0x") else int(value)
        for value in re.findall(r"0x[0-9a-fA-F]+|\b\d+\b", match.group("body"))
    ]
    if declared_len is not None and len(values) != declared_len:
        raise ValueError(
            f"byte array const {name!r} length mismatch in {path}: declared {declared_len}, got {len(values)}"
        )
    return values


def rust_const_inventory(path: Path, name_regex: str) -> list[str]:
    text = re.sub(r"/\*.*?\*/", "", path.read_text(), flags=re.S)
    text = re.sub(r"//.*", "", text)
    pattern = re.compile(name_regex)
    return [
        name
        for name in re.findall(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?const\s+([A-Z][A-Z0-9_]*)\s*:", text)
        if pattern.fullmatch(name)
    ]


def rust_const_value_as_int(value: Any) -> int:
    if isinstance(value, bool):
        raise ValueError(f"boolean const value {value!r} is not numeric")
    if isinstance(value, int):
        return value
    if isinstance(value, str):
        normalized = value.strip().replace("_", "")
        if normalized.startswith("0x"):
            return int(normalized, 16)
        if match := re.fullmatch(r"1\s*<<\s*(\d+)", normalized):
            return 1 << int(match.group(1))
        if re.fullmatch(r"-?\d+", normalized):
            return int(normalized)
    raise ValueError(f"const value {value!r} is not numeric")


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


def check_gpio_flag_mirrors(
    manifest: dict[str, Any],
    keyboard: dict[str, Any],
    zmk_config_dir: Path,
    project_root: Path,
) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("gpio_flag_mirrors", []):
        source_path = zmk_config_dir / check["source_file"]
        block = extract_block(source_path.read_text(), check["source_block"])
        entries = parse_gpio_property_with_flags(block, check["source_property"])
        index = int(check.get("source_index", 0))
        source_entry = entries[index] if index < len(entries) else ""
        expected_source = str(check["expected_source"])

        passed = 0
        total = 1
        messages: list[str] = []
        if source_entry == expected_source:
            passed += 1
        else:
            messages.append(f"source expected {expected_source!r}, got {source_entry!r}")

        try:
            source_pin, flags = gpio_entry_pin_and_flags(source_entry)
        except ValueError:
            source_pin, flags = "", set()

        for target_path in check.get("target_pin_paths", []):
            try:
                actual_pin = path_get(keyboard, target_path)
            except KeyError:
                actual_pin = None
            total += 1
            if actual_pin == source_pin:
                passed += 1
            else:
                messages.append(f"{target_path} expected {source_pin!r}, got {actual_pin!r}")

        if "target_low_active_path" in check:
            try:
                actual_low_active = path_get(keyboard, check["target_low_active_path"])
            except KeyError:
                actual_low_active = None
            expected_low_active = "GPIO_ACTIVE_LOW" in flags
            total += 1
            if actual_low_active == expected_low_active:
                passed += 1
            else:
                messages.append(
                    f"{check['target_low_active_path']} expected {expected_low_active!r}, "
                    f"got {actual_low_active!r}"
                )

        for flag in check.get("required_flags", []):
            total += 1
            if flag in flags:
                passed += 1
            else:
                messages.append(f"source missing GPIO flag {flag!r}")

        expected_needles = list(check.get("target_needles", []))
        if check.get("target_pin_needle"):
            expected_needles.append(source_pin)
        if check.get("target_active_low_needle") and "GPIO_ACTIVE_LOW" in flags:
            expected_needles.append(str(check["target_active_low_needle"]))
        if check.get("target_pull_up_needle") and "GPIO_PULL_UP" in flags:
            expected_needles.append(str(check["target_pull_up_needle"]))

        for target_file in check.get("target_files", []):
            needle_passed, missing = rust_text_contains_all(project_root, target_file, expected_needles)
            passed += needle_passed
            total += len(expected_needles)
            messages.extend(f"{target_file} missing {needle!r}" for needle in missing)

        results.append(
            Result(
                check["id"],
                "gpio_flag_mirror",
                passed,
                total,
                "ok" if not messages else "; ".join(messages),
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
                expected_source = str(
                    check.get(
                        "source_expected",
                        str(expected).lower() if isinstance(expected, bool) else expected,
                    )
                )
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


def contains_subsequence(values: list[int], expected: list[int]) -> bool:
    if not expected:
        return True
    return any(
        values[index : index + len(expected)] == expected
        for index in range(len(values) - len(expected) + 1)
    )


def check_rmk_patch_invariants(manifest: dict[str, Any], project_root: Path) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("rmk_patch_invariants", []):
        passed = 0
        total = 0
        messages: list[str] = []
        target_file = project_root / check["target_file"]

        byte_values: list[int] | None = None
        if "target_const" in check:
            total += 2
            try:
                byte_values = parse_rust_byte_array(target_file, check["target_const"])
            except (OSError, ValueError) as e:
                messages.append(str(e))
            else:
                expected_len = int(check["expected_len"])
                if len(byte_values) == expected_len:
                    passed += 1
                else:
                    messages.append(f"length expected {expected_len}, got {len(byte_values)}")

                actual_sha256 = hashlib.sha256(bytes(byte_values)).hexdigest()
                expected_sha256 = check["expected_sha256"]
                if actual_sha256 == expected_sha256:
                    passed += 1
                else:
                    messages.append(f"sha256 expected {expected_sha256}, got {actual_sha256}")

        for sequence in check.get("byte_sequences", []):
            total += 1
            expected_values = [int(value) for value in sequence["values"]]
            sequence_id = sequence.get("id", expected_values)
            if byte_values is not None and contains_subsequence(byte_values, expected_values):
                passed += 1
            else:
                messages.append(f"missing byte sequence {sequence_id!r}")

        if "needles" in check:
            try:
                text = target_file.read_text()
            except OSError as e:
                total += len(check["needles"])
                messages.append(str(e))
            else:
                for needle in check["needles"]:
                    total += 1
                    if needle in text:
                        passed += 1
                    else:
                        messages.append(f"missing needle {needle!r}")

        results.append(
            Result(
                check["id"],
                "rmk_patch",
                passed,
                total,
                "ok" if not messages else "; ".join(messages),
            )
        )
    return results


def rmk_lock_package(lock: dict[str, Any]) -> dict[str, Any] | None:
    for package in lock.get("package", []):
        if package.get("name") == "rmk":
            return package
    return None


def feature_set(value: Any) -> set[str]:
    return {str(item) for item in value}


def cargo_feature_closure(features: dict[str, Any], roots: list[str]) -> set[str]:
    seen: set[str] = set()
    stack = list(roots)
    while stack:
        feature = stack.pop()
        if feature in seen:
            continue
        seen.add(feature)
        for dependency in feature_set(features.get(feature, [])):
            dependency_feature = dependency.split("/", 1)[0].split("?", 1)[0]
            if dependency_feature in features and dependency_feature not in seen:
                stack.append(dependency_feature)
    return seen


def check_cargo_dependency_invariants(manifest: dict[str, Any], project_root: Path) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("cargo_dependency_invariants", []):
        passed = 0
        total = 0
        messages: list[str] = []

        try:
            cargo = load_toml(project_root / check.get("cargo_file", "Cargo.toml"))
            lock = load_toml(project_root / check.get("lock_file", "Cargo.lock"))
            vendor = load_toml(project_root / check["vendor_cargo_file"])
        except (OSError, tomllib.TOMLDecodeError) as e:
            results.append(Result(check["id"], "dependency", 0, 1, str(e)))
            continue

        total += 1
        expected_patch_path = check["patch_path"]
        try:
            actual_patch_path = path_list_get(cargo, ["patch", "crates-io", "rmk", "path"])
        except KeyError:
            messages.append("missing [patch.crates-io].rmk.path")
        else:
            if actual_patch_path == expected_patch_path:
                passed += 1
            else:
                messages.append(f"rmk patch path expected {expected_patch_path!r}, got {actual_patch_path!r}")

        total += 1
        if (project_root / expected_patch_path).is_dir():
            passed += 1
        else:
            messages.append(f"rmk patch path {expected_patch_path!r} is not a directory")

        expected_features = set(check["features"])
        vendor_features = vendor.get("features", {})
        for dep_id, dep_path, expected_optional in [
            ("top-level rmk dependency", ["dependencies", "rmk"], True),
            (
                "arm rmk dependency",
                ["target", 'cfg(target_arch = "arm")', "dependencies", "rmk"],
                False,
            ),
        ]:
            try:
                dependency = path_list_get(cargo, dep_path)
            except KeyError:
                total += 3
                messages.append(f"missing {dep_id}")
                continue

            total += 1
            actual_version = dependency.get("version")
            if actual_version == check["dependency_version"]:
                passed += 1
            else:
                messages.append(f"{dep_id} version expected {check['dependency_version']!r}, got {actual_version!r}")

            total += 1
            actual_features = feature_set(dependency.get("features", []))
            if actual_features == expected_features:
                passed += 1
            else:
                messages.append(
                    f"{dep_id} features expected {sorted(expected_features)!r}, got {sorted(actual_features)!r}"
                )

            total += 1
            actual_optional = bool(dependency.get("optional", False))
            if actual_optional == expected_optional:
                passed += 1
            else:
                messages.append(f"{dep_id} optional expected {expected_optional}, got {actual_optional}")

            total += 1
            expected_default_features = bool(check.get("default_features_enabled", True))
            actual_default_features = bool(dependency.get("default-features", True))
            if actual_default_features == expected_default_features:
                passed += 1
            else:
                messages.append(
                    f"{dep_id} default-features expected {expected_default_features}, got {actual_default_features}"
                )

        for implication in check.get("feature_closure_contains", []):
            feature = str(implication["feature"])
            expected_closure_items = [str(item) for item in implication["contains"]]
            closure = cargo_feature_closure(vendor_features, [feature])
            for expected_item in expected_closure_items:
                total += 1
                if expected_item in closure:
                    passed += 1
                else:
                    messages.append(
                        f"vendor rmk feature {feature!r} closure missing {expected_item!r}"
                    )

        bins_by_name = {str(entry.get("name", "")): entry for entry in cargo.get("bin", [])}
        for expected_bin in check.get("bins", []):
            bin_name = str(expected_bin["name"])
            actual_bin = bins_by_name.get(bin_name)
            total += 1
            if actual_bin is not None:
                passed += 1
            else:
                messages.append(f"missing [[bin]] {bin_name!r}")
                actual_bin = {}

            for field in ["path", "test", "bench"]:
                total += 1
                expected_value = expected_bin[field]
                actual_value = actual_bin.get(field)
                if actual_value == expected_value:
                    passed += 1
                else:
                    messages.append(
                        f"[[bin]] {bin_name}.{field} expected {expected_value!r}, got {actual_value!r}"
                    )

        total += 1
        actual_vendor_name = vendor.get("package", {}).get("name")
        actual_vendor_version = vendor.get("package", {}).get("version")
        if actual_vendor_name == "rmk" and actual_vendor_version == check["vendor_version"]:
            passed += 1
        else:
            messages.append(
                f"vendor package expected rmk {check['vendor_version']!r}, got {actual_vendor_name!r} {actual_vendor_version!r}"
            )

        lock_package = rmk_lock_package(lock)
        total += 1
        if lock_package and lock_package.get("version") == check["vendor_version"]:
            passed += 1
        else:
            messages.append(
                f"Cargo.lock rmk version expected {check['vendor_version']!r}, got {None if lock_package is None else lock_package.get('version')!r}"
            )

        total += 1
        if lock_package and "source" not in lock_package:
            passed += 1
        else:
            messages.append("Cargo.lock rmk package must resolve as the local path patch, not a registry source")

        results.append(
            Result(
                check["id"],
                "dependency",
                passed,
                total,
                "ok" if not messages else "; ".join(messages),
            )
        )
    return results


def check_makefile_task_invariants(manifest: dict[str, Any], project_root: Path) -> list[Result]:
    checks = list(manifest.get("makefile_task_invariants", []))
    if not checks:
        return []

    try:
        makefile = load_toml(project_root / "Makefile.toml")
    except (OSError, tomllib.TOMLDecodeError) as e:
        return [Result("makefile.toml", "build_task", 0, 1, str(e))]

    tasks = makefile.get("tasks", {})
    results: list[Result] = []
    for check in checks:
        task_name = str(check["task"])
        task = tasks.get(task_name)
        passed = 0
        total = 1
        messages: list[str] = []
        if isinstance(task, dict):
            passed += 1
        else:
            messages.append(f"missing [tasks.{task_name}]")
            task = {}

        for field in ["command"]:
            if field not in check:
                continue
            total += 1
            expected = check[field]
            actual = task.get(field)
            if actual == expected:
                passed += 1
            else:
                messages.append(f"tasks.{task_name}.{field} expected {expected!r}, got {actual!r}")

        for field, actual_field in [
            ("args_contains", "args"),
            ("dependencies_include", "dependencies"),
        ]:
            if field not in check:
                continue
            total += 1
            expected_values = list(check[field])
            actual_values = list(task.get(actual_field, []))
            missing = [value for value in expected_values if value not in actual_values]
            if not missing:
                passed += 1
            else:
                messages.append(f"tasks.{task_name}.{actual_field} missing required values {missing!r}")

        if "dependencies_equal" in check:
            total += 1
            expected_values = list(check["dependencies_equal"])
            actual_values = list(task.get("dependencies", []))
            if actual_values == expected_values:
                passed += 1
            else:
                messages.append(
                    f"tasks.{task_name}.dependencies expected {expected_values!r}, got {actual_values!r}"
                )

        if "script_contains" in check:
            total += 1
            expected_values = list(check["script_contains"])
            actual_script = str(task.get("script", ""))
            missing = [value for value in expected_values if value not in actual_script]
            if not missing:
                passed += 1
            else:
                messages.append(f"tasks.{task_name}.script missing required values {missing!r}")

        results.append(
            Result(
                check["id"],
                "build_task",
                passed,
                total,
                "ok" if not messages else "; ".join(messages),
            )
        )
    return results


def check_rust_const_inventories(manifest: dict[str, Any], project_root: Path) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("rust_const_inventories", []):
        expected = list(check["expected"])
        result_id = check["id"]
        try:
            actual = rust_const_inventory(
                project_root / check["target_file"], check["name_regex"]
            )
        except (OSError, ValueError, re.error) as e:
            results.append(
                Result(
                    result_id,
                    "rust_const_inventory",
                    0,
                    max(1, len(expected)),
                    str(e),
                )
            )
            continue
        results.append(
            ordered_inventory_result(result_id, "rust_const_inventory", expected, actual)
        )
    return results


def check_iqs9151_register_porting(
    manifest: dict[str, Any], project_root: Path
) -> list[Result]:
    return check_iqs9151_symbol_porting(
        manifest,
        project_root,
        inventory_key="iqs9151_register_addresses",
        manifest_key="iqs9151_register_porting",
        inventory_result_id="iqs9151_upstream_register_address_classification",
        result_prefix="iqs9151_register_porting",
        kind="iqs9151_register_porting",
        source_label="source register",
    )


def check_iqs9151_bit_porting(
    manifest: dict[str, Any], project_root: Path
) -> list[Result]:
    return check_iqs9151_symbol_porting(
        manifest,
        project_root,
        inventory_key="iqs9151_bit_flags",
        manifest_key="iqs9151_bit_porting",
        inventory_result_id="iqs9151_upstream_bit_flag_classification",
        result_prefix="iqs9151_bit_porting",
        kind="iqs9151_bit_porting",
        source_label="source bit flag",
    )


def check_iqs9151_symbol_porting(
    manifest: dict[str, Any],
    project_root: Path,
    *,
    inventory_key: str,
    manifest_key: str,
    inventory_result_id: str,
    result_prefix: str,
    kind: str,
    source_label: str,
) -> list[Result]:
    expected_inventory = list(manifest.get("source_inventory", {}).get(inventory_key, []))
    entries = list(manifest.get(manifest_key, []))
    actual_inventory = [
        f"{entry['source_const']}={int(entry['source_value'])}" for entry in entries
    ]
    results = [
        ordered_inventory_result(
            inventory_result_id,
            kind,
            expected_inventory,
            actual_inventory,
        )
    ]

    for entry in entries:
        source_const = entry["source_const"]
        source_value = int(entry["source_value"])
        status = entry["status"]
        result_id = f"{result_prefix}.{source_const}"
        passed = 0
        total = 1
        messages: list[str] = []

        if f"{source_const}={source_value}" in expected_inventory:
            passed += 1
        else:
            messages.append(f"{source_label} {source_const}={source_value} is not in inventory")

        if status == "ported":
            total += 1
            target_file = entry.get("target_file", "src/iqs9151.rs")
            target_const = entry["target_const"]
            try:
                actual_const = rust_const_value_as_int(
                    parse_rust_const(project_root / target_file, target_const)
                )
            except (OSError, ValueError) as e:
                messages.append(str(e))
            else:
                if actual_const == source_value:
                    passed += 1
                else:
                    messages.append(
                        f"{target_const} expected source value {source_value!r}, got {actual_const!r}"
                    )
        elif status in {"ported_by_behavior", "ported_by_config_image", "not_ported"}:
            total += 1
            reason = str(entry.get("reason", "")).strip()
            if reason:
                passed += 1
            else:
                messages.append(f"{source_const} has no reason for status {status!r}")
        else:
            messages.append(f"{source_const} has invalid status {status!r}")

        results.append(
            Result(
                result_id,
                kind,
                passed,
                total,
                "ok" if not messages else "; ".join(messages),
            )
        )

    return results


def collect_porting_status_entries(manifest: dict[str, Any]) -> list[dict[str, str]]:
    entries: list[dict[str, str]] = []
    for section, values in manifest.items():
        if not isinstance(values, list):
            continue
        for entry in values:
            if not isinstance(entry, dict) or "status" not in entry:
                continue
            source = str(entry.get("source_const", entry.get("id", "")))
            status = str(entry["status"])
            entries.append(
                {
                    "section": str(section),
                    "source": source,
                    "status": status,
                    "reason": str(entry.get("reason", "")).strip(),
                }
            )
    return entries


def porting_status_summary(manifest: dict[str, Any]) -> PortingStatusSummary:
    entries = collect_porting_status_entries(manifest)
    by_status = {status: 0 for status in sorted(VALID_PORTING_STATUSES)}
    remaining: list[dict[str, str]] = []
    implemented = 0
    for entry in entries:
        status = entry["status"]
        by_status[status] = by_status.get(status, 0) + 1
        if status in IMPLEMENTED_PORTING_STATUSES:
            implemented += 1
        else:
            remaining.append(entry)

    total = len(entries)
    return PortingStatusSummary(
        total=total,
        implemented=implemented,
        rate=None if total == 0 else implemented / total,
        by_status=by_status,
        remaining=remaining,
    )


def check_porting_status_evidence(
    manifest: dict[str, Any], results: list[Result]
) -> list[Result]:
    results_by_ref = {f"{result.kind}:{result.id}": result for result in results}
    evidence_results: list[Result] = []
    for entry in collect_porting_status_entries(manifest):
        status = entry["status"]
        if status not in {"ported_by_behavior", "ported_by_config_image"}:
            continue
        section = entry["section"]
        source = entry["source"]
        manifest_entries = manifest.get(section, [])
        manifest_entry = next(
            (
                candidate
                for candidate in manifest_entries
                if str(candidate.get("source_const", candidate.get("id", ""))) == source
            ),
            {},
        )
        evidence_refs = list(manifest_entry.get("evidence", []))
        if not evidence_refs:
            evidence_results.append(
                Result(
                    f"porting_status_evidence.{section}.{source}.missing",
                    "porting_status_evidence",
                    0,
                    1,
                    f"{section}.{source} has no evidence refs for status {status!r}",
                )
            )
            continue
        for index, ref in enumerate(evidence_refs):
            result_id = f"porting_status_evidence.{section}.{source}.{index}.{ref}"
            passed = 0
            messages: list[str] = []
            if not isinstance(ref, str) or ":" not in ref:
                messages.append(f"{section}.{source} evidence ref {ref!r} must be 'kind:id'")
            else:
                evidence = results_by_ref.get(ref)
                if evidence is None:
                    messages.append(
                        f"{section}.{source} evidence ref {ref!r} did not match any result"
                    )
                elif not evidence.ok:
                    messages.append(f"{section}.{source} evidence ref {ref!r} is failing")
                else:
                    passed = 1
            evidence_results.append(
                Result(
                    result_id,
                    "porting_status_evidence",
                    passed,
                    1,
                    "ok" if not messages else "; ".join(messages),
                )
            )
    return evidence_results


def coverage_by_kind(results: list[Result]) -> dict[str, CoverageBucket]:
    by_kind: dict[str, CoverageBucket] = {}
    for result in results:
        bucket = by_kind.setdefault(result.kind, CoverageBucket(0, 0, None))
        bucket.passed += result.passed
        bucket.total += result.total
    for bucket in by_kind.values():
        bucket.rate = None if bucket.total == 0 else bucket.passed / bucket.total
    return dict(sorted(by_kind.items()))


def result_inventory_digest(results: list[Result]) -> tuple[int, str]:
    entries = sorted(f"{result.kind}\t{result.id}\t{result.total}" for result in results)
    payload = "\n".join(entries) + "\n"
    return len(entries), hashlib.sha256(payload.encode()).hexdigest()


def compare_int_field(
    errors: list[str],
    label: str,
    expected_root: dict[str, Any],
    actual_root: dict[str, int],
    field: str,
) -> None:
    if field not in expected_root:
        errors.append(f"{label}: baseline missing field {field}")
        return
    expected = int(expected_root[field])
    actual = int(actual_root[field])
    if actual != expected:
        errors.append(f"{label}.{field}: expected baseline {expected}, got {actual}")


def baseline_errors(
    baseline: dict[str, Any],
    passed: int,
    total: int,
    by_kind: dict[str, CoverageBucket],
    status_summary: PortingStatusSummary,
    result_count: int,
    result_inventory_sha256: str,
) -> list[str]:
    errors: list[str] = []
    coverage = baseline.get("coverage", {})
    if not isinstance(coverage, dict):
        return ["coverage baseline must contain a [coverage] table"]
    compare_int_field(errors, "coverage", coverage, {"passed": passed, "total": total}, "passed")
    compare_int_field(errors, "coverage", coverage, {"passed": passed, "total": total}, "total")
    compare_int_field(
        errors,
        "coverage",
        coverage,
        {"result_count": result_count},
        "result_count",
    )
    expected_sha256 = str(coverage.get("result_inventory_sha256", "")).strip()
    if not expected_sha256:
        errors.append("coverage: baseline missing field result_inventory_sha256")
    elif result_inventory_sha256 != expected_sha256:
        errors.append(
            "coverage.result_inventory_sha256: "
            f"expected baseline {expected_sha256}, got {result_inventory_sha256}"
        )

    expected_by_kind = coverage.get("by_kind", {})
    if not isinstance(expected_by_kind, dict) or not expected_by_kind:
        errors.append("coverage baseline must contain [coverage.by_kind.*] tables")
    else:
        actual_kind_names = set(by_kind)
        expected_kind_names = set(expected_by_kind)
        for kind in sorted(expected_kind_names - actual_kind_names):
            errors.append(f"coverage.by_kind.{kind}: baseline kind is missing from actual report")
        for kind in sorted(actual_kind_names - expected_kind_names):
            errors.append(f"coverage.by_kind.{kind}: actual report kind is missing from baseline")
        for kind in sorted(expected_kind_names & actual_kind_names):
            expected = expected_by_kind[kind]
            if not isinstance(expected, dict):
                errors.append(f"coverage.by_kind.{kind}: baseline entry must be a table")
                continue
            bucket = by_kind[kind]
            actual = {"passed": bucket.passed, "total": bucket.total}
            compare_int_field(errors, f"coverage.by_kind.{kind}", expected, actual, "passed")
            compare_int_field(errors, f"coverage.by_kind.{kind}", expected, actual, "total")

    expected_status = baseline.get("porting_status", {})
    if not isinstance(expected_status, dict):
        errors.append("coverage baseline must contain a [porting_status] table")
    else:
        actual_status = {
            "total": status_summary.total,
            "implemented": status_summary.implemented,
        }
        compare_int_field(errors, "porting_status", expected_status, actual_status, "total")
        compare_int_field(
            errors,
            "porting_status",
            expected_status,
            actual_status,
            "implemented",
        )
        expected_by_status = expected_status.get("by_status", {})
        if not isinstance(expected_by_status, dict) or not expected_by_status:
            errors.append("coverage baseline must contain [porting_status.by_status]")
        else:
            actual_status_names = set(status_summary.by_status)
            expected_status_names = set(expected_by_status)
            for status in sorted(expected_status_names - actual_status_names):
                errors.append(
                    f"porting_status.by_status.{status}: baseline status is missing from actual report"
                )
            for status in sorted(actual_status_names - expected_status_names):
                errors.append(
                    f"porting_status.by_status.{status}: actual status is missing from baseline"
                )
            for status in sorted(expected_status_names & actual_status_names):
                expected = int(expected_by_status[status])
                actual = int(status_summary.by_status[status])
                if actual != expected:
                    errors.append(
                        f"porting_status.by_status.{status}: expected baseline {expected}, got {actual}"
                    )

    return errors


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


def check_code_topology(manifest: dict[str, Any], project_root: Path) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("code_topology", []):
        text = (project_root / check["file"]).read_text()
        required = list(check.get("required", []))
        forbidden = list(check.get("forbidden", []))
        passed_required = sum(1 for needle in required if needle in text)
        passed_forbidden = sum(1 for needle in forbidden if needle not in text)
        messages = [f"missing {needle!r}" for needle in required if needle not in text]
        messages.extend(f"forbidden {needle!r} is present" for needle in forbidden if needle in text)
        results.append(
            Result(
                check["id"],
                "code_topology",
                passed_required + passed_forbidden,
                len(required) + len(forbidden),
                "ok" if not messages else "; ".join(messages),
            )
        )
    return results


def check_file_contains_invariants(manifest: dict[str, Any], project_root: Path) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("file_contains_invariants", []):
        target_file = str(check["file"])
        needles = list(check["needles"])
        try:
            text = (project_root / target_file).read_text()
        except OSError as e:
            results.append(Result(check["id"], "release_workflow", 0, len(needles), str(e)))
            continue
        passed = sum(1 for needle in needles if needle in text)
        missing = [needle for needle in needles if needle not in text]
        results.append(
            Result(
                check["id"],
                "release_workflow",
                passed,
                len(needles),
                "ok" if not missing else f"{target_file} missing {missing!r}",
            )
        )
    return results


def extract_rust_function_scope(text: str, function_name: str) -> str | None:
    match = re.search(
        rf"(?m)^\s*(?:pub(?:\s*\([^)]*\))?\s+)?(?:async\s+)?fn\s+{re.escape(function_name)}\s*\(",
        text,
    )
    if not match:
        return None
    open_brace = text.find("{", match.end())
    if open_brace == -1:
        return None

    depth = 0
    for index in range(open_brace, len(text)):
        char = text[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return text[match.start() : index + 1]
    return None


def extract_rust_function_attributes(text: str, function_name: str) -> str | None:
    match = re.search(
        rf"(?m)^\s*(?:pub(?:\s*\([^)]*\))?\s+)?(?:async\s+)?fn\s+{re.escape(function_name)}\s*\(",
        text,
    )
    if not match:
        return None
    line_start = text.rfind("\n", 0, match.start()) + 1
    attr_start = line_start
    while attr_start > 0:
        previous_end = attr_start - 1
        previous_start = text.rfind("\n", 0, previous_end) + 1
        previous_line = text[previous_start:previous_end].strip()
        if (
            previous_line.startswith("#[")
            or previous_line.startswith("//")
            or not previous_line
        ):
            attr_start = previous_start
            continue
        break
    return text[attr_start:line_start]


def rust_attributes_contain_test(attributes: str | None) -> bool:
    return attributes is not None and re.search(r"#\s*\[\s*test\b", attributes) is not None


def rust_attributes_contain_ignore(attributes: str | None) -> bool:
    return attributes is not None and re.search(r"#\s*\[[^\]]*\bignore\b", attributes) is not None


def check_rust_unit_tests(manifest: dict[str, Any], project_root: Path) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("rust_unit_tests", []):
        target_file = str(check["file"])
        function_name = str(check["function"])
        needles = list(check["needles"])
        total = len(needles) + 2
        try:
            text = (project_root / target_file).read_text()
        except OSError as e:
            results.append(Result(check["id"], "rust_unit_test", 0, total, str(e)))
            continue
        function_scope = extract_rust_function_scope(text, function_name)
        attributes = extract_rust_function_attributes(text, function_name)
        if function_scope is None:
            results.append(
                Result(
                    check["id"],
                    "rust_unit_test",
                    0,
                    total,
                    f"{target_file} missing function {function_name!r}",
                )
            )
            continue
        passed = 0
        messages: list[str] = []
        if rust_attributes_contain_test(attributes):
            passed += 1
        else:
            messages.append(f"{target_file}::{function_name} must be marked #[test]")
        if not rust_attributes_contain_ignore(attributes):
            passed += 1
        else:
            messages.append(f"{target_file}::{function_name} must not be ignored")
        missing = [needle for needle in needles if needle not in function_scope]
        passed += sum(1 for needle in needles if needle in function_scope)
        messages.extend(
            f"{target_file}::{function_name} missing {needle!r}" for needle in missing
        )
        results.append(
            Result(
                check["id"],
                "rust_unit_test",
                passed,
                total,
                "ok" if not messages else "; ".join(messages),
            )
        )
    return results


def check_runtime_scenario_tests(manifest: dict[str, Any], project_root: Path) -> list[Result]:
    results: list[Result] = []
    for check in manifest.get("runtime_scenario_tests", []):
        target_file = str(check["file"])
        needles = list(check["needles"])
        function_name = check.get("function")
        try:
            text = (project_root / target_file).read_text()
        except OSError as e:
            results.append(Result(check["id"], "runtime_scenario", 0, len(needles), str(e)))
            continue
        scope = text
        scope_description = target_file
        if function_name:
            function_scope = extract_rust_function_scope(text, str(function_name))
            if function_scope is None:
                results.append(
                    Result(
                        check["id"],
                        "runtime_scenario",
                        0,
                        len(needles),
                        f"{target_file} missing function {function_name!r}",
                    )
                )
                continue
            scope = function_scope
            scope_description = f"{target_file}::{function_name}"
        passed = sum(1 for needle in needles if needle in scope)
        missing = [needle for needle in needles if needle not in scope]
        results.append(
            Result(
                check["id"],
                "runtime_scenario",
                passed,
                len(needles),
                "ok" if not missing else f"{scope_description} missing {missing!r}",
            )
        )
    return results


def rust_trackpad_button_order(text: str) -> list[str]:
    match = re.search(
        r"const\s+TRACKPAD_BUTTONS_BY_INPUT_CODE\s*:\s*\[TrackpadButton;\s*\d+\]\s*=\s*\[(.*?)\];",
        text,
        re.S,
    )
    if not match:
        return []
    return re.findall(r"TrackpadButton::([A-Za-z0-9_]+)", match.group(1))


def rust_trackpad_input_btn_codes(text: str) -> dict[str, int]:
    match = re.search(
        r"pub\s+const\s+fn\s+input_btn_code\s*\(\s*self\s*\)\s*->\s*u8\s*\{(.*?)\n\s*\}",
        text,
        re.S,
    )
    if not match:
        return {}
    return {
        item.group(1): int(item.group(2))
        for item in re.finditer(r"Self::([A-Za-z0-9_]+)\s*=>\s*(\d+)", match.group(1))
    }


def rust_trackpad_positions(text: str, const_name: str) -> list[tuple[int, int]]:
    match = re.search(
        rf"const\s+{re.escape(const_name)}\s*:\s*\[VirtualKeyPosition;\s*\d+\]\s*=\s*\[(.*?)\];",
        text,
        re.S,
    )
    if not match:
        return []
    return [
        (int(item.group(1)), int(item.group(2)))
        for item in re.finditer(
            r"VirtualKeyPosition\s*\{\s*row:\s*(\d+),\s*col:\s*(\d+)\s*\}",
            match.group(1),
        )
    ]


def rust_trackpad_position_match_arms(text: str) -> dict[str, str]:
    match = re.search(
        r"pub\s+const\s+fn\s+trackpad_button_position\s*\([^)]*\)\s*->\s*VirtualKeyPosition\s*\{(.*?)\n\}",
        text,
        re.S,
    )
    if not match:
        return {}
    return {
        item.group(1): item.group(2)
        for item in re.finditer(
            r"TrackpadSide::([A-Za-z0-9_]+)\s*=>\s*([A-Z_]+_TRACKPAD_BUTTON_POSITIONS)\s*\[\s*index\s*\]",
            match.group(1),
        )
    }


def check_trackpad_virtual_buttons(
    manifest: dict[str, Any], config: dict[str, Any], project_root: Path
) -> list[Result]:
    expected = list(manifest.get("trackpad_virtual_buttons", []))
    if not expected:
        return []

    iqs_text = (project_root / "src/iqs9151.rs").read_text()
    button_order = rust_trackpad_button_order(iqs_text)
    input_btn_codes = rust_trackpad_input_btn_codes(iqs_text)
    positions_by_side = {
        "left": rust_trackpad_positions(iqs_text, "LEFT_TRACKPAD_BUTTON_POSITIONS"),
        "right": rust_trackpad_positions(iqs_text, "RIGHT_TRACKPAD_BUTTON_POSITIONS"),
    }
    position_match_arms = rust_trackpad_position_match_arms(iqs_text)
    runtime_positions: dict[tuple[str, str], tuple[int, int]] = {}
    for side, positions in positions_by_side.items():
        for button in button_order:
            index = input_btn_codes.get(button)
            if index is not None and index < len(positions):
                runtime_positions[(side, button)] = positions[index]

    km = keymap(config)
    vial_positions = set(collect_vial_positions(load_json(project_root / "vial.json")["layouts"]["keymap"]))
    results: list[Result] = []
    expected_button_order = [str(entry["button"]) for entry in expected[: len(button_order)]]
    button_order_total = max(len(expected_button_order), len(button_order))
    button_order_passed = sum(
        1
        for index in range(button_order_total)
        if (expected_button_order[index] if index < len(expected_button_order) else None)
        == (button_order[index] if index < len(button_order) else None)
    )
    button_order_messages = [
        f"i{index}: expected {expected_button_order[index] if index < len(expected_button_order) else None!r}, "
        f"got {button_order[index] if index < len(button_order) else None!r}"
        for index in range(button_order_total)
        if (expected_button_order[index] if index < len(expected_button_order) else None)
        != (button_order[index] if index < len(button_order) else None)
    ]
    results.append(
        Result(
            "trackpad_virtual_button_input_order",
            "trackpad_virtual",
            button_order_passed,
            button_order_total,
            "ok" if not button_order_messages else "; ".join(button_order_messages[:8]),
        )
    )

    expected_codes = {str(entry["button"]): index for index, entry in enumerate(expected[: len(button_order)])}
    code_total = len(expected_codes)
    code_passed = sum(1 for button, code in expected_codes.items() if input_btn_codes.get(button) == code)
    code_messages = [
        f"{button}: expected input_btn_code {code}, got {input_btn_codes.get(button)!r}"
        for button, code in expected_codes.items()
        if input_btn_codes.get(button) != code
    ]
    results.append(
        Result(
            "trackpad_virtual_button_input_btn_codes",
            "trackpad_virtual",
            code_passed,
            code_total,
            "ok" if not code_messages else "; ".join(code_messages[:8]),
        )
    )

    match_arm_checks = [
        ("Left", position_match_arms.get("Left"), "LEFT_TRACKPAD_BUTTON_POSITIONS"),
        ("Right", position_match_arms.get("Right"), "RIGHT_TRACKPAD_BUTTON_POSITIONS"),
    ]
    match_arm_passed = sum(1 for _, actual, want in match_arm_checks if actual == want)
    match_arm_messages = [
        f"{side}: expected {want!r}, got {actual!r}"
        for side, actual, want in match_arm_checks
        if actual != want
    ]
    results.append(
        Result(
            "trackpad_virtual_button_side_position_match_arms",
            "trackpad_virtual",
            match_arm_passed,
            len(match_arm_checks),
            "ok" if not match_arm_messages else "; ".join(match_arm_messages),
        )
    )

    for entry in expected:
        side = str(entry["side"])
        button = str(entry["button"])
        expected_position = (int(entry["row"]), int(entry["col"]))
        expected_action = str(entry["action"])
        actual_position = runtime_positions.get((side, button))
        actual_action = None
        if actual_position is not None:
            row, col = actual_position
            if row < len(km[0]) and col < len(km[0][row]):
                actual_action = km[0][row][col]

        passed = 0
        messages: list[str] = []
        if actual_position == expected_position:
            passed += 1
        else:
            messages.append(f"runtime position expected {expected_position!r}, got {actual_position!r}")
        if actual_action == expected_action:
            passed += 1
        else:
            messages.append(f"layer0 action expected {expected_action!r}, got {actual_action!r}")
        if expected_position in vial_positions:
            passed += 1
        else:
            messages.append(f"Vial layout missing {expected_position!r}")

        results.append(
            Result(
                f"trackpad_virtual_button.{side}.{button}",
                "trackpad_virtual",
                passed,
                3,
                "ok" if not messages else "; ".join(messages),
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


def vial_kle_geometry_signature(value: Any) -> str:
    def normalize(item: Any) -> Any:
        if isinstance(item, str):
            return "<key>" if re.fullmatch(r"\d+,\d+", item) else item
        if isinstance(item, list):
            return [normalize(child) for child in item]
        if isinstance(item, dict):
            return {str(key): normalize(item[key]) for key in sorted(item)}
        return item

    payload = json.dumps(normalize(value), sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(payload.encode()).hexdigest()


def keyboard_toml_vial_positions(config: dict[str, Any]) -> list[tuple[int, int]]:
    layers = keymap(config)
    rows = int(config["layout"]["rows"])
    cols = int(config["layout"]["cols"])
    positions: list[tuple[int, int]] = []
    for row in range(rows):
        for col in range(cols):
            actions = [
                layers[layer][row][col]
                for layer in range(len(layers))
                if row < len(layers[layer]) and col < len(layers[layer][row])
            ]
            if any(action not in (NO_KEY, TRANSPARENT) for action in actions):
                positions.append((row, col))
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


def check_vial_keyboard_toml_layout(
    manifest: dict[str, Any], config: dict[str, Any], project_root: Path
) -> list[Result]:
    vial = load_json(project_root / "vial.json")
    expected_layout = manifest["layout"]
    results: list[Result] = []

    def normalized_hex_u16(value: Any) -> str:
        if isinstance(value, int):
            return f"0x{value:04X}"
        if isinstance(value, str):
            try:
                return f"0x{int(value.strip(), 0):04X}"
            except ValueError:
                return value
        return str(value)

    identity_checks = [
        ("name", vial.get("name"), path_get(config, "keyboard.name")),
        (
            "vendorId",
            normalized_hex_u16(vial.get("vendorId")),
            normalized_hex_u16(path_get(config, "keyboard.vendor_id")),
        ),
        (
            "productId",
            normalized_hex_u16(vial.get("productId")),
            normalized_hex_u16(path_get(config, "keyboard.product_id")),
        ),
    ]
    identity_passed = sum(1 for _, actual, expected in identity_checks if actual == expected)
    identity_messages = [
        f"{name} expected {expected!r}, got {actual!r}"
        for name, actual, expected in identity_checks
        if actual != expected
    ]
    results.append(
        Result(
            "vial_identity_matches_keyboard_toml",
            "vial",
            identity_passed,
            len(identity_checks),
            "ok" if not identity_messages else "; ".join(identity_messages),
        )
    )

    serial_number = str(path_get(config, "keyboard.serial_number"))
    serial_prefix_ok = serial_number.startswith("vial:f64c2b3c:")
    results.append(
        Result(
            "vial_keyboard_serial_number_prefix",
            "vial",
            1 if serial_prefix_ok else 0,
            1,
            "ok"
            if serial_prefix_ok
            else f"keyboard.serial_number must start with 'vial:f64c2b3c:', got {serial_number!r}",
        )
    )

    matrix_checks = [
        ("rows", vial["matrix"].get("rows"), expected_layout["rows"]),
        ("cols", vial["matrix"].get("cols"), expected_layout["cols"]),
    ]
    matrix_passed = sum(1 for _, actual, expected in matrix_checks if actual == expected)
    matrix_messages = [
        f"{name} expected {expected!r}, got {actual!r}"
        for name, actual, expected in matrix_checks
        if actual != expected
    ]
    results.append(
        Result(
            "vial_keyboard_toml_matrix_shape",
            "vial",
            matrix_passed,
            len(matrix_checks),
            "ok" if not matrix_messages else "; ".join(matrix_messages),
        )
    )

    required_positions = keyboard_toml_vial_positions(config)
    actual_positions = collect_vial_positions(vial["layouts"]["keymap"])
    expected_geometry_sha = manifest.get("layout", {}).get("vial_kle_geometry_sha256")
    actual_geometry_sha = vial_kle_geometry_signature(vial["layouts"]["keymap"])
    geometry_ok = actual_geometry_sha == expected_geometry_sha
    results.append(
        Result(
            "vial_kle_geometry_signature",
            "vial",
            1 if geometry_ok else 0,
            1,
            "ok"
            if geometry_ok
            else f"vial.json KLE geometry sha256 expected {expected_geometry_sha!r}, got {actual_geometry_sha!r}",
        )
    )
    allowed_no_action_positions = {
        (int(position["row"]), int(position["col"]))
        for position in manifest.get("layout", {}).get("vial_allowed_no_action_positions", [])
    }
    expected_exposed_positions = set(required_positions) | allowed_no_action_positions
    rows = int(config["layout"]["rows"])
    cols = int(config["layout"]["cols"])
    bound_messages: list[str] = []
    in_bounds = 0
    for index, actual in enumerate(actual_positions):
        if 0 <= actual[0] < rows and 0 <= actual[1] < cols:
            in_bounds += 1
        else:
            bound_messages.append(f"p{index}: {actual!r} is outside keyboard.toml bounds {rows}x{cols}")
    duplicate_positions = sorted(
        position for position in set(actual_positions) if actual_positions.count(position) > 1
    )
    unique_passed = not duplicate_positions
    if duplicate_positions:
        bound_messages.append(f"duplicate Vial positions {duplicate_positions!r}")
    results.append(
        Result(
            "vial_positions_within_keyboard_toml_bounds",
            "vial",
            in_bounds + (1 if unique_passed else 0),
            len(actual_positions) + 1,
            "ok" if not bound_messages else "; ".join(bound_messages[:8]),
        )
    )

    actual_set = set(actual_positions)
    missing_required = [position for position in required_positions if position not in actual_set]
    required_passed = len(required_positions) - len(missing_required)
    results.append(
        Result(
            "vial_positions_cover_keyboard_toml_actions",
            "vial",
            required_passed,
            len(required_positions),
            "ok"
            if not missing_required
            else f"missing keyboard.toml action positions {missing_required[:8]!r}",
        )
    )
    missing_exposed = sorted(expected_exposed_positions - actual_set)
    extra_exposed = sorted(actual_set - expected_exposed_positions)
    exact_total = len(expected_exposed_positions | actual_set)
    exact_passed = len(expected_exposed_positions & actual_set)
    exact_mismatches = []
    if missing_exposed:
        exact_mismatches.append(f"missing expected exposed positions {missing_exposed[:8]!r}")
    if extra_exposed:
        exact_mismatches.append(f"unexpected Vial positions {extra_exposed[:8]!r}")
    results.append(
        Result(
            "vial_positions_match_keyboard_toml_exposed_positions",
            "vial",
            exact_passed,
            exact_total,
            "ok" if not exact_mismatches else "; ".join(exact_mismatches[:8]),
        )
    )
    results.extend(check_vial_thumb_layer_taps(manifest, config, actual_positions))
    return results


def check_vial_thumb_layer_taps(
    manifest: dict[str, Any],
    config: dict[str, Any],
    vial_positions: list[tuple[int, int]],
) -> list[Result]:
    results: list[Result] = []
    km = keymap(config)
    actual_position_set = set(vial_positions)
    for entry in manifest.get("layout", {}).get("vial_thumb_layer_taps", []):
        row = int(entry["row"])
        col = int(entry["col"])
        expected_action = str(entry["expected_action"])
        expected_layer = int(entry["activates_layer"])
        expected_tap = str(entry["tap"])
        expected_profile = str(entry["profile"])
        actual_action = (
            km[0][row][col]
            if 0 <= row < len(km[0]) and 0 <= col < len(km[0][row])
            else None
        )
        actual_layer, actual_tap, actual_profile = (
            layer_tap_parts(actual_action) if isinstance(actual_action, str) else (None, None, None)
        )
        checks = [
            (
                "Vial position",
                (row, col) in actual_position_set,
                f"Vial layout missing {(row, col)!r}",
            ),
            (
                "action",
                actual_action == expected_action,
                f"action expected {expected_action!r}, got {actual_action!r}",
            ),
            (
                "tap",
                actual_tap == expected_tap,
                f"tap expected {expected_tap!r}, got {actual_tap!r}",
            ),
            (
                "layer",
                actual_layer == expected_layer,
                f"hold layer expected {expected_layer}, got {actual_layer!r}",
            ),
            (
                "profile",
                actual_profile == expected_profile,
                f"profile expected {expected_profile!r}, got {actual_profile!r}",
            ),
        ]
        messages = [message for _, ok, message in checks if not ok]
        results.append(
            Result(
                f"vial_thumb_layer_tap.{entry['id']}",
                "vial",
                len(checks) - len(messages),
                len(checks),
                "ok" if not messages else "; ".join(messages),
            )
        )
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

    expected_items = list(
        manifest.get(
            "vial_custom_keycodes",
            manifest.get("layout", {}).get("vial_custom_keycodes", []),
        )
    )
    actual_items = list(vial.get("customKeycodes", []))
    custom_total = max(len(expected_items), len(actual_items))
    custom_passed = 0
    custom_mismatches: list[str] = []
    for index in range(custom_total):
        expected = expected_items[index] if index < len(expected_items) else None
        actual = actual_items[index] if index < len(actual_items) else None
        if isinstance(expected, str):
            expected = {"name": expected}
        if isinstance(actual, str):
            actual = {"name": actual}
        if not isinstance(expected, dict) or not isinstance(actual, dict):
            custom_mismatches.append(f"c{index}: expected {expected!r}, got {actual!r}")
            continue
        fields = [field for field in ("name", "title", "shortName") if field in expected]
        passed_fields = sum(1 for field in fields if actual.get(field) == expected.get(field))
        custom_passed += passed_fields
        for field in fields:
            if actual.get(field) != expected.get(field):
                custom_mismatches.append(
                    f"c{index}.{field}: expected {expected.get(field)!r}, got {actual.get(field)!r}"
                )
    custom_total = sum(
        len([field for field in ("name", "title", "shortName") if isinstance(item, dict) and field in item])
        if isinstance(item, dict)
        else 1
        for item in expected_items
    )
    if len(actual_items) != len(expected_items):
        custom_mismatches.append(
            f"custom key count expected {len(expected_items)}, got {len(actual_items)}"
        )
        custom_total += abs(len(actual_items) - len(expected_items))
    results.append(
        Result(
            "vial_custom_keycodes_match_user_key_labels",
            "vial",
            custom_passed,
            custom_total,
            "ok" if not custom_mismatches else "; ".join(custom_mismatches),
        )
    )
    return results


def user_key_index(key: str) -> int:
    match = re.fullmatch(r"User(\d+)", key)
    if not match:
        raise ValueError(f"invalid RMK user key {key!r}")
    return int(match.group(1))


def check_vial_user_key_semantics(
    manifest: dict[str, Any],
    keyboard: dict[str, Any],
    project_root: Path,
) -> list[Result]:
    vial = load_json(project_root / "vial.json")
    actual_names = [item.get("name") for item in vial.get("customKeycodes", [])]
    handler_text = (project_root / "vendor/rmk-0.8.2/src/keyboard.rs").read_text()
    profile_count = int(path_get(keyboard, "rmk.ble_profiles_num"))
    expected = list(
        manifest.get("layout", {}).get("vial_user_key_semantics", [])
    )
    results: list[Result] = []

    for entry in expected:
        key = str(entry["key"])
        name = str(entry["name"])
        try:
            index = user_key_index(key)
        except ValueError as e:
            results.append(Result(f"vial_user_key_semantics.{key}", "vial_user_key_semantics", 0, 1, str(e)))
            continue

        passed = 0
        total = 2 + len(entry.get("handler_needles", []))
        messages: list[str] = []
        actual_name = actual_names[index] if index < len(actual_names) else None
        if actual_name == name:
            passed += 1
        else:
            messages.append(f"{key} expected Vial name {name!r}, got {actual_name!r}")

        expected_profile_count = sum(
            1
            for semantic in expected
            if re.fullmatch(r"BT\d+", str(semantic.get("name", "")))
        )
        if profile_count == expected_profile_count:
            passed += 1
        else:
            messages.append(
                f"rmk.ble_profiles_num expected {expected_profile_count}, got {profile_count}"
            )

        for needle in entry.get("handler_needles", []):
            if needle in handler_text:
                passed += 1
            else:
                messages.append(f"missing handler needle {needle!r}")

        results.append(
            Result(
                f"vial_user_key_semantics.{key}.{name}",
                "vial_user_key_semantics",
                passed,
                total,
                "ok" if not messages else "; ".join(messages),
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
    results.extend(check_gpio_flag_mirrors(manifest, keyboard, zmk_config_dir, project_root))
    results.extend(check_source_regex_values(manifest, zmk_config_dir))
    results.extend(check_zmk_matrix_transform(manifest, zmk_config_dir))
    results.extend(check_rust_const_values(manifest, zmk_config_dir, project_root))
    results.extend(check_rust_const_inventories(manifest, project_root))
    results.extend(check_iqs9151_register_porting(manifest, project_root))
    results.extend(check_iqs9151_bit_porting(manifest, project_root))
    results.extend(check_rust_byte_arrays(manifest, project_root))
    results.extend(check_rmk_patch_invariants(manifest, project_root))
    results.extend(check_cargo_dependency_invariants(manifest, project_root))
    results.extend(check_vial_layout(manifest, project_root, zmk_config_dir))
    results.extend(check_vial_user_key_semantics(manifest, keyboard, project_root))
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
    candidates: list[Path] = []
    if hint:
        candidates.append(Path(hint) / "config" / "lalapadgen2.keymap")
        candidates.append(Path(Path(hint).name) / "config" / "lalapadgen2.keymap")
    for candidate in candidates:
        if candidate.exists():
            return candidate
    if candidates:
        return candidates[0]
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
    results.extend(check_split_footprint(manifest, keyboard))
    results.extend(check_behavior_values(manifest, keyboard))
    results.extend(check_combos(manifest, keyboard))
    results.extend(check_scenarios(manifest, keyboard))
    results.extend(check_code_contains(manifest, project_root))
    results.extend(check_code_topology(manifest, project_root))
    results.extend(check_file_contains_invariants(manifest, project_root))
    results.extend(check_rust_unit_tests(manifest, project_root))
    results.extend(check_runtime_scenario_tests(manifest, project_root))
    results.extend(check_makefile_task_invariants(manifest, project_root))
    results.extend(check_trackpad_virtual_buttons(manifest, keyboard, project_root))
    results.extend(check_vial_keyboard_toml_layout(manifest, keyboard, project_root))
    results.extend(
        check_zmk_source(
            manifest,
            keyboard,
            project_root,
            zmk_keymap_path if zmk_keymap_path is not None else default_zmk_keymap_path(manifest),
            require_zmk_source,
        )
    )
    results.extend(check_porting_status_evidence(manifest, results))
    return results


def print_text(results: list[Result], status_summary: PortingStatusSummary) -> None:
    passed = sum(result.passed for result in results)
    total = sum(result.total for result in results)
    rate = 100.0 if total == 0 else passed * 100.0 / total
    print(f"Porting coverage: {passed}/{total} = {rate:.2f}%")
    by_kind = coverage_by_kind(results)
    if by_kind:
        print("Porting coverage by kind:")
        for kind, bucket in by_kind.items():
            bucket_rate = (
                "n/a" if bucket.rate is None else f"{bucket.rate * 100.0:.2f}%"
            )
            print(f"- {kind}: {bucket.passed}/{bucket.total} = {bucket_rate}")
    if status_summary.rate is not None:
        status_rate = status_summary.rate * 100.0
        status_parts = ", ".join(
            f"{status}={count}"
            for status, count in sorted(status_summary.by_status.items())
            if count
        )
        print(
            "Porting status: "
            f"{status_summary.implemented}/{status_summary.total} = {status_rate:.2f}% "
            f"implemented ({status_parts})"
        )
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
    parser.add_argument(
        "--require-porting-complete",
        action="store_true",
        help="Fail if any explicit manifest status is not implemented.",
    )
    parser.add_argument(
        "--coverage-baseline",
        type=Path,
        default=None,
        help="Fail if the coverage denominator, result-id inventory, or implementation-status snapshot drifts.",
    )
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)

    results = run(args.manifest, args.keyboard_toml, args.zmk_keymap, args.require_zmk_source)
    manifest = load_toml(args.manifest)
    status_summary = porting_status_summary(manifest)
    passed = sum(result.passed for result in results)
    total = sum(result.total for result in results)
    by_kind = coverage_by_kind(results)
    result_count, result_sha256 = result_inventory_digest(results)
    baseline_failures: list[str] = []
    if args.coverage_baseline is not None:
        try:
            baseline = load_toml(args.coverage_baseline)
        except OSError as e:
            baseline_failures = [f"failed to read {args.coverage_baseline}: {e}"]
        else:
            baseline_failures = baseline_errors(
                baseline,
                passed,
                total,
                by_kind,
                status_summary,
                result_count,
                result_sha256,
            )
    if args.json:
        print(
            json.dumps(
                {
                    "passed": passed,
                    "total": total,
                    "rate": None if total == 0 else passed / total,
                    "result_count": result_count,
                    "result_inventory_sha256": result_sha256,
                    "by_kind": {
                        kind: {
                            "passed": bucket.passed,
                            "total": bucket.total,
                            "rate": bucket.rate,
                        }
                        for kind, bucket in by_kind.items()
                    },
                    "porting_status": {
                        "total": status_summary.total,
                        "implemented": status_summary.implemented,
                        "rate": status_summary.rate,
                        "by_status": status_summary.by_status,
                        "remaining": status_summary.remaining,
                    },
                    "baseline_errors": baseline_failures,
                    "results": [result.__dict__ | {"ok": result.ok} for result in results],
                },
                indent=2,
                sort_keys=True,
            )
        )
    else:
        print_text(results, status_summary)

    ok = passed == total
    if args.require_porting_complete and status_summary.implemented != status_summary.total:
        print(
            "porting status incomplete: "
            f"{status_summary.implemented}/{status_summary.total} explicit statuses implemented",
            file=sys.stderr,
        )
        ok = False
    if baseline_failures:
        print("porting coverage baseline drift:", file=sys.stderr)
        for failure in baseline_failures:
            print(f"- {failure}", file=sys.stderr)
        ok = False
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
