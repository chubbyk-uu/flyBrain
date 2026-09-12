"""Audit the restored arrays and exact browser/source inputs without changing them."""
from __future__ import annotations

import hashlib
import json
import subprocess
from datetime import UTC, datetime
from pathlib import Path

import numpy as np

from flybrain.connectome import PackedConnectome

ROOT = Path(__file__).resolve().parents[1]


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    root = ROOT / "outputs/packs/male_cns_v1"
    pack = PackedConnectome.load(root)
    io = json.loads((ROOT / "assets/neuromechfly/male_cns_v1_neural_io.json").read_text())
    arrays = {name: sha(root / name) for name in pack.manifest["array_sha256"]}
    assert arrays == pack.manifest["array_sha256"] == io["dataset"]["pack_array_sha256"]
    counts = {
        "neurons": pack.neuron_count, "edges": pack.edge_count,
        "contacts": int(np.abs(pack.signed_counts.astype(np.int64)).sum()),
        "excitatory_edges": int((pack.signed_counts > 0).sum()),
        "inhibitory_edges": int((pack.signed_counts < 0).sum()),
    }
    assert all(value == pack.manifest["counts"][name] for name, value in counts.items())
    ids = set(map(int, pack.neuron_ids))
    assert len(ids) == pack.neuron_count
    selected = {value for group in io["groups"].values() for value in group["root_ids"]}
    assert not selected - ids
    resources = json.loads((ROOT / "web/dist/runtime-assets.json").read_text())["files"]
    assert all(sha(ROOT / "assets/neuromechfly" / name) == value for name, value in resources.items())
    runtime = ["web/dist/runtime-assets.json", "web/dist/flybrain.js",
               "web/dist/flybrain_browser.wasm", "web/neural-engine.js", "web/neural.wgsl",
               "web/simulation-worker.js", "web/scene.js", "web/perf-test.js",
               "web/neural-test.js", "web/package-lock.json", "Cargo.lock",
               "rust/src/brain_bridge.rs", "rust/src/metal_engine.rs",
               "rust/src/browser_engine.rs", "rust/src/parameters.rs"]
    report = {
        "status": "PASS", "recorded_utc": datetime.now(UTC).isoformat(),
        "commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "pack_source": "https://flybrain.mehran.dk/pack/manifest.json",
        "manifest_sha256": sha(root / "manifest.json"), "arrays_sha256": arrays,
        "arrays_bytes": {name: (root / name).stat().st_size for name in arrays},
        "counts": counts, "io_groups": len(io["groups"]), "io_unique_neurons": len(selected),
        "io_missing_neurons": 0, "resources_sha256": resources,
        "runtime_sha256": {name: sha(ROOT / name) for name in runtime},
        "transport": "Published manifest and chunk files preserved; full NPY arrays reassembled verbatim",
    }
    output = ROOT / "outputs/baselines/rtx5080-windows/assets-audit.json"
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("x") as stream:
        json.dump(report, stream, indent=2)
        stream.write("\n")
    print(json.dumps({"status": "PASS", "counts": counts, "resources": len(resources),
                      "io_unique_neurons": len(selected)}))


if __name__ == "__main__":
    main()
