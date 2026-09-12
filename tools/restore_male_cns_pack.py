"""Restore the published pack verbatim, validating it against the pinned I/O artifact."""
from __future__ import annotations

import hashlib
import json
import subprocess
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import numpy as np

from flybrain.connectome import PackedConnectome

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / "outputs/packs/male_cns_v1"
BASE = "https://flybrain.mehran.dk/pack/"


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def download(name: str, expected: str | None = None) -> bytes:
    if any(part in ("", ".", "..") for part in name.split("/")) or any(
        char in name for char in "\\:%?#"
    ):
        raise ValueError(f"Unsafe asset path: {name}")
    # curl inherits the configured shell proxy; the host rejects urllib's client.
    data = subprocess.check_output(
        ["curl", "--fail", "--silent", "--show-error", "--location",
         "--retry", "3", "--max-time", "120", BASE + name]
    )
    if expected is not None and digest(data) != expected:
        raise ValueError(f"SHA-256 mismatch: {name}")
    return data


def store_verified(name: str, data: bytes) -> None:
    path = DEST / name
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        if path.read_bytes() != data:
            raise FileExistsError(f"Refusing to replace different content: {path}")
        return
    with path.open("xb") as stream:
        stream.write(data)


def main() -> None:
    io_path = ROOT / "assets/neuromechfly/male_cns_v1_neural_io.json"
    pinned = json.loads(io_path.read_text())["dataset"]
    raw_manifest = download("manifest.json")
    manifest = json.loads(raw_manifest)
    assert manifest["array_sha256"] == pinned["pack_array_sha256"]
    assert manifest["source_sha256"]["annotations"] == pinned["annotation_sha256"]
    assert manifest["materialization"] == pinned["materialization"]
    assert (manifest["neuron_count"], manifest["edge_count"]) == (166700, 24469412)

    def restore(item: tuple[str, str]) -> None:
        name, expected = item
        transport = manifest.get("browser_chunks", {}).get(name)
        if transport:
            chunks = []
            for part in transport["parts"]:
                chunk = download(part["path"], part["sha256"])
                assert len(chunk) == part["bytes"]
                store_verified(part["path"], chunk)
                chunks.append(chunk)
            data = b"".join(chunks)
            assert len(data) == transport["bytes"]
            assert digest(data) == expected
        else:
            data = download(name, expected)
        store_verified(name, data)
        print(f"verified {name}: {len(data)} bytes {expected}", flush=True)

    with ThreadPoolExecutor(max_workers=4) as pool:
        list(pool.map(restore, manifest["array_sha256"].items()))
    # Preserve the published manifest, including its transport-only chunk metadata.
    store_verified("manifest.json", raw_manifest)
    pack = PackedConnectome.load(DEST)
    assert np.unique(pack.neuron_ids).size == pack.neuron_count
    assert sum(abs(pack.signed_counts.astype(np.int64))) == manifest["contact_sum"]
    for name, dtype in (("neuron_ids", "uint64"), ("row_ptr", "uint32"),
                        ("destinations", "uint32"), ("signed_counts", "int16")):
        assert getattr(pack, name).dtype == np.dtype(dtype)
    print(f"PASS: {pack.neuron_count} neurons, {pack.edge_count} edges; manifest {digest(raw_manifest)}")


if __name__ == "__main__":
    main()
