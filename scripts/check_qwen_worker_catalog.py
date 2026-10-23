#!/usr/bin/env python3
"""No-weights/no-audio startup checks for both explicit Qwen worker identities."""
import argparse
import json
import os
import pathlib
import subprocess
import tempfile


def probe(worker, ort, directory, model=None, expect_ready=True):
    command = [str(worker), str(directory), "6"]
    if model:
        command += ["--model", model]
    env = dict(os.environ, ORT_DYLIB_PATH=str(ort), OPENBLAS_NUM_THREADS="6",
               OMP_NUM_THREADS="6", MKL_NUM_THREADS="6", MEETILY_DISABLE_ANALYTICS="1",
               HF_HUB_DISABLE_TELEMETRY="1", DO_NOT_TRACK="1")
    # communicate has a hard bound; no device, transcript or inference request.
    process = subprocess.Popen(command, env=env, stdin=subprocess.PIPE,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    try:
        stdout, _ = process.communicate('{"op":"shutdown","id":"catalog-probe"}\n', timeout=40)
    except subprocess.TimeoutExpired:
        process.kill()
        process.communicate()
        raise AssertionError("worker startup probe timed out")
    if not expect_ready:
        assert process.returncode != 0
        assert not any(json.loads(line).get("event") == "ready" for line in stdout.splitlines())
        return
    assert process.returncode == 0
    ready, bye = [json.loads(line) for line in stdout.splitlines()]
    assert ready["event"] == "ready" and ready["protocol"] == 1
    assert ready["model"] == (model or "Qwen3-ASR-0.6B")
    assert ready["model_loaded"] is False and ready["threads"] == 6
    assert ready["recognition_context"] == "full_input"
    assert ready["vad_only"] is False and "float32" in ready["audio_formats"]
    assert bye["status"] == "bye"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--worker", required=True, type=pathlib.Path)
    parser.add_argument("--ort", required=True, type=pathlib.Path)
    args = parser.parse_args()
    worker, ort = args.worker.resolve(strict=True), args.ort.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="qwen-catalog-no-audio-") as name:
        root = pathlib.Path(name)
        missing = root / "no-weights"
        for model in ["Qwen3-ASR-1.7B", "Qwen3-ASR-0.6B"]:
            probe(worker, ort, missing, model)
        small = root / "small-config-only"
        small.mkdir()
        (small / "config.json").write_text(json.dumps({"model_type": "qwen3_asr",
            "thinker_config": {"text_config": {"hidden_size": 1024, "num_hidden_layers": 28}}}))
        probe(worker, ort, small)
        probe(worker, ort, small, "Qwen3-ASR-1.7B", expect_ready=False)
        probe(worker, ort, missing, "unknown", expect_ready=False)
        probe(worker, ort, missing, expect_ready=False)
    print(json.dumps({"passed": True, "conditions": 6, "model_weights_loaded": False,
                      "audio_used": False, "inference_used": False}))


if __name__ == "__main__":
    main()
