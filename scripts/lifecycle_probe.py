#!/usr/bin/env python3
"""Run a prebuilt hello-axion lifecycle-probe binary; never build or reuse EventLoop."""

import argparse
import datetime
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import queue
import re
import subprocess
import tempfile
import threading
import time

SCHEMA = "axion.lifecycle-probe.v1"
SCENARIOS = ("normal-close", "partial-window-fail", "resource-fail", "render-fail")


def redact(value):
    return re.sub(
        r"([?&](?:token|bridge_token|bridgeToken)=)[^&\s\"']+",
        r"\1[redacted]",
        value,
    )


def sample_process(pid):
    if os.name != "posix":
        return {"available": False, "reason": "RSS sampling requires POSIX ps"}
    try:
        result = subprocess.run(
            ["/bin/ps", "-p", str(pid), "-o", "rss="],
            capture_output=True,
            text=True,
            timeout=1,
        )
        if result.returncode or not result.stdout.strip():
            return {"available": False, "reason": "ps could not observe the child"}
        sample = {"available": True, "rss_kib": int(result.stdout.strip())}
        if platform.system() == "Darwin":
            threads = subprocess.run(
                ["/bin/ps", "-M", "-p", str(pid)],
                capture_output=True,
                text=True,
                timeout=1,
            )
            if threads.returncode == 0:
                sample["threads"] = max(0, len(threads.stdout.splitlines()) - 1)
        return sample
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        return {"available": False, "reason": type(error).__name__}


def validate_records(records, scenario, windows):
    failures = []
    if any(not isinstance(row, dict) for row in records):
        return ["probe records must be JSON objects"]
    if any(row.get("scenario") != scenario for row in records):
        failures.append("probe records did not match the requested scenario")
    if any(not isinstance(row.get("detail"), dict) for row in records):
        return failures + ["probe record detail must be a JSON object"]
    backend = [
        row for row in records
        if row.get("source") == "backend" and row.get("phase") == "backend.finished"
    ]
    host = [
        row for row in records
        if row.get("source") == "host" and row.get("phase") == "backend.returned"
    ]
    if len(backend) != 1 or len(host) != 1:
        return ["expected exactly one backend completion and one host return"]
    detail = backend[0].get("detail", {})
    for key in ("state_observed", "state_released"):
        if detail.get(key) is not True:
            failures.append(key + " did not match the release contract")
    for key in ("window_registry_count", "webview_policy_count"):
        if type(detail.get(key)) is not int or detail[key] != 0:
            failures.append(key + " did not match the release contract")
    if not any(row.get("phase") == "state.drop_begin" for row in records):
        failures.append("AppState drop was not observed")
    expected_ok = scenario not in ("partial-window-fail", "render-fail")
    if detail.get("result_ok") is not expected_ok:
        failures.append("backend result did not match the scenario")
    if host[0].get("detail", {}).get("result_ok") is not expected_ok:
        failures.append("host result did not match the scenario")
    peak = detail.get("peak_window_count")
    if type(peak) is not int:
        failures.append("peak window count must be an integer")
    elif scenario == "partial-window-fail":
        if peak != 1:
            failures.append("partial failure must follow exactly one registered window")
    elif peak != windows:
        failures.append("not all expected windows were registered")
    if expected_ok:
        confirmed = detail.get("close_confirmed_count")
        if type(confirmed) is not int or confirmed < windows:
            failures.append("explicit close confirmation was not observed for every window")
    else:
        kind = "partial-window" if scenario == "partial-window-fail" else "render"
        if not any(
            row.get("phase") == "failure.injected"
            and row.get("detail", {}).get("kind") == kind
            and row.get("detail", {}).get("synthetic") is True
            for row in records
        ):
            failures.append("the expected synthetic failure was not observed")
    frontend = [
        row.get("detail", {}).get("record", {})
        for row in records if row.get("phase") == "frontend.record"
    ]
    if any(not isinstance(row, dict) for row in frontend):
        return failures + ["frontend probe record must be a JSON object"]
    if any(row.get("phase") == "frontend.error" for row in frontend):
        failures.append("frontend probe reported an error")
    if scenario == "resource-fail":
        if detail.get("resource_failure_observed") is not True:
            failures.append("backend did not observe the missing asset failure")
        if not any(row.get("phase") == "resource.failure" for row in frontend):
            failures.append("frontend did not observe the resource failure before close")
    return failures


def run_case(executable, scenario, windows=1, hold_ms=0, timeout=30, sample=True):
    windows = 2 if scenario == "partial-window-fail" else windows
    records, samples, stderr = [], [], []
    messages = queue.Queue()
    started = time.monotonic()
    sent, heartbeats = {}, []
    timed_out = False
    quit_sent = False
    protocol_error = None
    ignored_stdout_lines = 0
    with tempfile.TemporaryDirectory(prefix="axion-lifecycle-probe-") as temporary:
        env = os.environ.copy()
        for name in ("AXION_GUI_SMOKE", "AXION_SELFTEST_BRIDGE", "AXION_EXIT_AFTER_STARTUP"):
            env.pop(name, None)
        env.update({
            "AXION_LIFECYCLE_PROBE": scenario,
            "AXION_LIFECYCLE_PROBE_WINDOWS": str(windows),
            "AXION_LIFECYCLE_PROBE_HOLD_MS": str(hold_ms),
            "AXION_LIFECYCLE_PROBE_DATA_DIR": str(Path(temporary).resolve() / "data"),
        })
        child = subprocess.Popen(
            [str(executable)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, text=True, encoding="utf-8", errors="replace", env=env,
        )

        def collect(stream, source):
            for line in stream:
                messages.put((source, line))
            messages.put((source, None))

        readers = [
            threading.Thread(target=collect, args=(child.stdout, "stdout"), daemon=True),
            threading.Thread(target=collect, args=(child.stderr, "stderr"), daemon=True),
        ]
        for reader in readers:
            reader.start()
        deadline = started + timeout
        next_sample = started
        ended = set()
        while True:
            now = time.monotonic()
            if now >= deadline:
                timed_out = True
                break
            if sample and now >= next_sample and child.poll() is None:
                item = sample_process(child.pid)
                item["observed_ms"] = (now - started) * 1000
                samples.append(item)
                next_sample = now + 0.25
            try:
                source, line = messages.get(timeout=min(0.05, deadline - now))
            except queue.Empty:
                if child.poll() is not None and len(ended) == 2:
                    break
                continue
            if line is None:
                ended.add(source)
                continue
            if source == "stderr":
                stderr.append(redact(line.rstrip()))
                del stderr[:-80]
                continue
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                ignored_stdout_lines += 1
                continue
            if not isinstance(event, dict) or event.get("schema") != SCHEMA:
                ignored_stdout_lines += 1
                continue
            # Probe records only contain counts/phase data. Do not retain unrelated engine output.
            event["observed_ms"] = (time.monotonic() - started) * 1000
            records.append(event)
            detail = event.get("detail")
            if not isinstance(detail, dict):
                protocol_error = "probe record detail must be a JSON object"
                break
            if event.get("source") == "host" and event.get("phase") == "backend.returned":
                if sample:
                    item = sample_process(child.pid)
                    item["observed_ms"] = (time.monotonic() - started) * 1000
                    item["phase"] = "backend.returned"
                    samples.append(item)
                sent[1] = time.monotonic()
                try:
                    child.stdin.write("PING 1\n")
                    child.stdin.flush()
                except OSError:
                    protocol_error = "host stopped accepting post-backend heartbeats"
                    break
            if event.get("source") == "host" and event.get("phase") == "heartbeat":
                sequence = detail.get("sequence")
                if type(sequence) is int and sequence in sent and sequence == len(heartbeats) + 1:
                    heartbeats.append({
                        "sequence": sequence,
                        "response_ms": (time.monotonic() - sent[sequence]) * 1000,
                    })
                    try:
                        if sequence < 3:
                            sent[sequence + 1] = time.monotonic()
                            child.stdin.write("PING " + str(sequence + 1) + "\n")
                        else:
                            child.stdin.write("QUIT\n")
                        child.stdin.flush()
                        quit_sent = sequence == 3
                    except OSError:
                        protocol_error = "host stopped accepting post-backend heartbeats"
                        break
        if child.poll() is None:
            child.kill()
        return_code = child.wait()
        for reader in readers:
            reader.join(timeout=1)
        for stream in (child.stdin, child.stdout, child.stderr):
            try:
                stream.close()
            except OSError:
                pass
    failures = validate_records(records, scenario, windows)
    if timed_out:
        failures.append("isolated child exceeded the watchdog timeout")
    if protocol_error:
        failures.append(protocol_error)
    if len(heartbeats) != 3 or not quit_sent:
        failures.append("same host did not answer three post-backend heartbeats")
    if return_code != 0:
        failures.append("host exited unsuccessfully")
    phases = [
        row for row in records
        if row.get("phase") == "frontend.record"
        and isinstance(row.get("detail"), dict)
        and isinstance(row["detail"].get("record"), dict)
        and row["detail"]["record"].get("phase") == "close.requested"
    ]
    returned = [row for row in records if row.get("phase") == "backend.returned"]
    rss = [row["rss_kib"] for row in samples if row.get("available")]
    return {
        "scenario": scenario, "pid": child.pid, "windows": windows,
        "result": "fail" if failures else "ok", "failures": failures,
        "timeout": timed_out, "return_code": return_code,
        "wall_ms": (time.monotonic() - started) * 1000,
        "last_phase": records[-1].get("phase") if records else None,
        "close_to_backend_return_observed_ms": (
            returned[-1]["observed_ms"] - phases[0]["observed_ms"]
            if phases and returned else None
        ),
        "rss_peak_observed_kib": max(rss) if rss else None,
        "heartbeats": heartbeats, "samples": samples, "records": records,
        "stderr_tail": stderr[-80:], "ignored_stdout_lines": ignored_stdout_lines,
    }


def file_sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--scenario", choices=("all",) + SCENARIOS, default="all")
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--windows", type=int, choices=(1, 2), default=1)
    parser.add_argument("--hold-ms", type=int, default=0)
    parser.add_argument("--timeout-seconds", type=float, default=30)
    parser.add_argument("--no-process-sampling", action="store_true")
    parser.add_argument(
        "--output", type=Path, default=Path("target/axion/reports/lifecycle-probe.json")
    )
    args = parser.parse_args()
    if not 1 <= args.repetitions <= 20 or not 0 <= args.hold_ms <= 60000:
        parser.error("repetitions must be 1..20 and hold-ms must be 0..60000")
    if (not math.isfinite(args.timeout_seconds) or args.timeout_seconds <= 0
            or not args.executable.is_file() or not os.access(args.executable, os.X_OK)):
        parser.error("provide an existing prebuilt executable and a positive timeout")
    executable = args.executable.resolve()
    scenarios = SCENARIOS if args.scenario == "all" else (args.scenario,)
    report = {
        "schema": "axion.lifecycle-probe-suite.v1",
        "measured_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "binary": {"path": str(executable), "sha256": file_sha256(executable)},
        "platform": {"system": platform.system(), "release": platform.release(),
                     "architecture": platform.machine(), "macos": platform.mac_ver()[0]},
        "sampling": {"cache": "uncontrolled; separate processes", "rss_unit": "KiB",
                     "rss_kind": "observed ps RSS, not OS high-water mark",
                     "hold_ms": args.hold_ms, "repetitions": args.repetitions},
        "cases": [],
    }
    for scenario in scenarios:
        for iteration in range(args.repetitions):
            case = run_case(
                executable, scenario, args.windows, args.hold_ms,
                args.timeout_seconds, not args.no_process_sampling,
            )
            case["iteration"] = iteration + 1
            report["cases"].append(case)
            print(scenario + " #" + str(iteration + 1) + ": " + case["result"], flush=True)
    report["result"] = (
        "ok" if all(case["result"] == "ok" for case in report["cases"]) else "fail"
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print("report: " + str(args.output))
    return 0 if report["result"] == "ok" else 1


if __name__ == "__main__":
    raise SystemExit(main())
