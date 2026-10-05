"""Driver protocol fixtures; these tests do not start Servo or prove its release."""

import copy
import importlib.util
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch


DRIVER = Path(__file__).resolve().parents[1] / "lifecycle_probe.py"
SPEC = importlib.util.spec_from_file_location("lifecycle_probe", DRIVER)
probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(probe)


def records_for(scenario="normal-close", windows=1):
    expected_ok = scenario not in ("partial-window-fail", "render-fail")

    def event(source, phase, detail):
        return {"schema": probe.SCHEMA, "scenario": scenario,
                "source": source, "phase": phase, "detail": detail}

    rows = [event("backend", "state.drop_begin", {"windows_before_clear": windows})]
    if not expected_ok:
        kind = "partial-window" if scenario == "partial-window-fail" else "render"
        rows.append(event("backend", "failure.injected", {"kind": kind, "synthetic": True}))
    if scenario == "resource-fail":
        rows.append(event("host", "frontend.record", {"record": {"phase": "resource.failure"}}))
    rows.extend([
        event("backend", "backend.finished", {
            "state_observed": True, "state_released": True,
            "window_registry_count": 0, "webview_policy_count": 0,
            "peak_window_count": 1 if scenario == "partial-window-fail" else windows,
            "close_confirmed_count": windows if expected_ok else 0,
            "resource_failure_observed": scenario == "resource-fail",
            "result_ok": expected_ok,
        }),
        event("host", "backend.returned", {"result_ok": expected_ok}),
    ])
    return rows


class ValidationTests(unittest.TestCase):
    def test_all_scenarios_have_distinct_result_contracts(self):
        for scenario in probe.SCENARIOS:
            with self.subTest(scenario=scenario):
                self.assertEqual(probe.validate_records(records_for(scenario, 2), scenario, 2), [])

    def test_live_state_and_nonempty_bindings_fail(self):
        for key, value in (("state_released", False), ("window_registry_count", 1),
                           ("webview_policy_count", 1)):
            with self.subTest(key=key):
                rows = records_for()
                rows[-2]["detail"][key] = value
                self.assertTrue(probe.validate_records(rows, "normal-close", 1))

    def test_counts_do_not_accept_booleans_or_null(self):
        for key in ("window_registry_count", "webview_policy_count",
                    "peak_window_count", "close_confirmed_count"):
            for value in (False, True, None, "0"):
                with self.subTest(key=key, value=value):
                    rows = records_for()
                    rows[-2]["detail"][key] = value
                    self.assertTrue(probe.validate_records(rows, "normal-close", 1))

    def test_synthetic_failure_requires_label(self):
        rows = records_for("render-fail")
        rows[1]["detail"]["synthetic"] = False
        self.assertTrue(probe.validate_records(rows, "render-fail", 1))

    def test_resource_failure_needs_both_sides(self):
        rows = records_for("resource-fail")
        rows[-2]["detail"]["resource_failure_observed"] = False
        self.assertTrue(probe.validate_records(rows, "resource-fail", 1))
        rows = records_for("resource-fail")
        del rows[1]
        self.assertTrue(probe.validate_records(rows, "resource-fail", 1))

    def test_malformed_records_fail_without_type_error(self):
        for row in (None, {"detail": []}, {"detail": {"record": []},
                                         "phase": "frontend.record", "scenario": "normal-close"}):
            with self.subTest(row=row):
                rows = records_for() + [copy.deepcopy(row)]
                self.assertTrue(probe.validate_records(rows, "normal-close", 1))

    def test_mismatched_scenario_fails(self):
        rows = records_for()
        rows[0]["scenario"] = "resource-fail"
        self.assertTrue(probe.validate_records(rows, "normal-close", 1))

    def test_query_tokens_are_redacted(self):
        self.assertEqual(probe.redact("https://x/?token=secret&bridge_token=private&ok=1"),
                         "https://x/?token=[redacted]&bridge_token=[redacted]&ok=1")


FAKE_CHILD = r'''
import json
import os
import sys
import time

scenario = os.environ["AXION_LIFECYCLE_PROBE"]
windows = int(os.environ["AXION_LIFECYCLE_PROBE_WINDOWS"])

def emit(source, phase, detail):
    print(json.dumps({"schema": "axion.lifecycle-probe.v1", "source": source,
                      "scenario": scenario, "phase": phase, "detail": detail}), flush=True)

emit("host", "host.started", {"windows": windows})
emit("backend", "state.drop_begin", {"windows_before_clear": windows})
if MODE == "hang":
    time.sleep(60)
if MODE == "malformed":
    emit("host", "heartbeat", [])
    time.sleep(60)
emit("host", "frontend.record", {"record": {"phase": "close.requested"}})
emit("backend", "backend.finished", {
    "state_observed": True, "state_released": True,
    "window_registry_count": 0, "webview_policy_count": 0,
    "peak_window_count": windows, "close_confirmed_count": windows,
    "resource_failure_observed": False, "result_ok": True,
})
if MODE == "early-exit":
    os.close(0)
emit("host", "backend.returned", {"result_ok": True})
if MODE == "early-exit":
    raise SystemExit(0)
for line in sys.stdin:
    if line.strip() == "QUIT":
        break
    emit("host", "heartbeat", {"sequence": int(line.split()[1])})
emit("host", "host.done", {})
'''


@unittest.skipUnless(os.name == "posix", "executable Python fixtures require POSIX")
class DriverTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="axion-probe-driver-test-")
        self.addCleanup(self.temporary.cleanup)

    def executable(self, mode):
        path = Path(self.temporary.name) / (mode + ".py")
        path.write_text("#!" + sys.executable + "\nMODE = " + repr(mode) + "\n" + FAKE_CHILD)
        path.chmod(0o700)
        return path

    def test_separate_children_answer_three_heartbeats(self):
        binary = self.executable("normal")
        first = probe.run_case(binary, "normal-close", timeout=3, sample=False)
        second = probe.run_case(binary, "normal-close", windows=2, timeout=3, sample=False)
        self.assertEqual(first["result"], "ok", first["failures"])
        self.assertEqual(second["result"], "ok", second["failures"])
        self.assertNotEqual(first["pid"], second["pid"])
        self.assertEqual([row["sequence"] for row in first["heartbeats"]], [1, 2, 3])
        self.assertIsNotNone(first["close_to_backend_return_observed_ms"])

    def test_early_exit_is_a_reported_failure(self):
        case = probe.run_case(self.executable("early-exit"), "normal-close", timeout=3, sample=False)
        self.assertEqual(case["result"], "fail")
        self.assertIn("same host did not answer three post-backend heartbeats", case["failures"])

    def test_host_return_has_an_immediate_sample(self):
        with patch.object(probe, "sample_process", side_effect=lambda pid: {"available": True, "rss_kib": 123}) as sampler:
            case = probe.run_case(self.executable("normal"), "normal-close", timeout=3, sample=True)
        self.assertEqual(case["result"], "ok", case["failures"])
        marked = [row for row in case["samples"] if row.get("phase") == "backend.returned"]
        self.assertEqual(len(marked), 1)
        self.assertEqual(marked[0]["rss_kib"], 123)
        self.assertGreaterEqual(sampler.call_count, 2)

    def test_watchdog_preserves_last_phase(self):
        case = probe.run_case(self.executable("hang"), "normal-close", timeout=1, sample=False)
        self.assertTrue(case["timeout"])
        self.assertEqual(case["last_phase"], "state.drop_begin")
        self.assertLess(case["wall_ms"], 4000)

    def test_malformed_live_event_is_a_reported_failure(self):
        case = probe.run_case(self.executable("malformed"), "normal-close", timeout=3, sample=False)
        self.assertEqual(case["result"], "fail")
        self.assertFalse(case["timeout"])
        self.assertIn("probe record detail must be a JSON object", case["failures"])


if __name__ == "__main__":
    unittest.main()
