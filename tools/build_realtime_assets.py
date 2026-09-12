#!/usr/bin/env python3
"""Build a separate, auditable NeuroMechFly realtime asset tree."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import shutil
import sys
import xml.etree.ElementTree as ET
from collections import Counter
from pathlib import Path

import mujoco

COLLISION_ATTRIBUTES = (
    "condim",
    "friction",
    "gap",
    "margin",
    "priority",
    "solimp",
    "solmix",
    "solref",
)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def format_vector(values) -> str:
    return " ".join(f"{float(value):.12g}" for value in values)


def rotate_vector(quaternion, vector) -> list[float]:
    w, x, y, z = map(float, quaternion)
    vx, vy, vz = map(float, vector)
    # Unit-quaternion rotation without introducing a NumPy dependency here.
    tx = 2.0 * (y * vz - z * vy)
    ty = 2.0 * (z * vx - x * vz)
    tz = 2.0 * (x * vy - y * vx)
    return [
        vx + w * tx + (y * tz - z * ty),
        vy + w * ty + (z * tx - x * tz),
        vz + w * tz + (x * ty - y * tx),
    ]


def quaternion_conjugate(quaternion) -> list[float]:
    w, x, y, z = map(float, quaternion)
    return [w, -x, -y, -z]


def quaternion_product(left, right) -> list[float]:
    lw, lx, ly, lz = map(float, left)
    rw, rx, ry, rz = map(float, right)
    return [
        lw * rw - lx * rx - ly * ry - lz * rz,
        lw * rx + lx * rw + ly * rz - lz * ry,
        lw * ry - lx * rz + ly * rw + lz * rx,
        lw * rz + lx * ry - ly * rx + lz * rw,
    ]


def model_counts(model: mujoco.MjModel) -> dict[str, int]:
    return {
        "qpos": int(model.nq),
        "dofs": int(model.nv),
        "bodies": int(model.nbody),
        "joints": int(model.njnt),
        "actuators": int(model.nu),
        "sensors": int(model.nsensor),
        "geoms": int(model.ngeom),
    }


def collidable_fly_meshes(model: mujoco.MjModel) -> list[int]:
    result = []
    for geom_id in range(model.ngeom):
        name = mujoco.mj_id2name(model, mujoco.mjtObj.mjOBJ_GEOM, geom_id) or ""
        if (
            name.startswith("fly/")
            and int(model.geom_type[geom_id]) == int(mujoco.mjtGeom.mjGEOM_MESH)
            and int(model.geom_contype[geom_id]) != 0
        ):
            result.append(geom_id)
    return result


def fly_collision_types(model: mujoco.MjModel) -> dict[str, int]:
    result: Counter[str] = Counter()
    for geom_id in range(model.ngeom):
        name = mujoco.mj_id2name(model, mujoco.mjtObj.mjOBJ_GEOM, geom_id) or ""
        if name.startswith("fly/") and int(model.geom_contype[geom_id]) != 0:
            kind = mujoco.mjtGeom(int(model.geom_type[geom_id])).name
            result[kind.removeprefix("mjGEOM_").lower()] += 1
    return dict(sorted(result.items()))


def proxy_kind(name: str) -> str:
    if any(token in name for token in ("coxa", "trochanterfemur", "tibia", "tarsus")):
        return "capsule"
    return "ellipsoid"


def build_proxy_attributes(
    model: mujoco.MjModel, geom_id: int, source: ET.Element
) -> dict[str, str]:
    name = source.attrib["name"]
    half_sizes = [float(value) for value in model.geom_aabb[geom_id, 3:]]
    center = [float(value) for value in model.geom_aabb[geom_id, :3]]
    quaternion = [float(value) for value in model.geom_quat[geom_id]]
    rotated_center = rotate_vector(quaternion, center)
    position = [float(model.geom_pos[geom_id, axis]) + rotated_center[axis] for axis in range(3)]
    kind = proxy_kind(name)
    if kind == "capsule":
        if max(range(3), key=half_sizes.__getitem__) != 2:
            raise ValueError(f"processed limb mesh {name} is not aligned to its local z axis")
        radius = max(half_sizes[0], half_sizes[1])
        half_length = max(half_sizes[2] - radius, radius * 0.05)
        size = [radius, half_length]
    else:
        size = half_sizes
    attributes = {
        "name": name,
        "type": kind,
        "size": format_vector(size),
        "pos": format_vector(position),
        "quat": format_vector(quaternion),
        "mass": "0",
        "contype": source.get("contype", "1"),
        "conaffinity": source.get("conaffinity", "0"),
        "group": "4",
        "rgba": "0 0 0 0",
        "fluidshape": "none",
    }
    for attribute in COLLISION_ATTRIBUTES:
        if attribute in source.attrib:
            attributes[attribute] = source.attrib[attribute]
    return attributes


def freeze_fully_passive_bodies(
    compiled: mujoco.MjModel,
    root: ET.Element,
    policy: str,
) -> list[str]:
    actuators = root.find("actuator")
    driven_joints = {
        actuator.get("joint")
        for actuator in ([] if actuators is None else list(actuators))
        if actuator.get("joint")
    }
    reference = mujoco.MjData(compiled)
    mujoco.mj_resetDataKeyframe(compiled, reference, 0)
    mujoco.mj_forward(compiled, reference)
    removed = []
    for body in root.findall(".//body"):
        joints = body.findall("joint")
        if not joints or any(joint.get("type") == "free" for joint in joints):
            continue
        names = [joint.get("name") for joint in joints]
        if any(name in driven_joints for name in names):
            continue
        body_name = body.get("name")
        if body_name is None:
            raise ValueError("passive body is missing a name")
        if policy == "nonleg" and "tarsus" in body_name:
            continue
        if policy == "terminal-feet" and body_name.endswith("tarsus5"):
            continue
        body_id = mujoco.mj_name2id(compiled, mujoco.mjtObj.mjOBJ_BODY, body_name)
        if body_id < 1:
            raise ValueError(f"cannot resolve passive body {body_name}")
        parent_id = int(compiled.body_parentid[body_id])
        parent_quaternion = reference.xquat[parent_id]
        delta = [
            float(reference.xpos[body_id, axis] - reference.xpos[parent_id, axis])
            for axis in range(3)
        ]
        local_position = rotate_vector(quaternion_conjugate(parent_quaternion), delta)
        local_quaternion = quaternion_product(
            quaternion_conjugate(parent_quaternion), reference.xquat[body_id]
        )
        body.set("pos", format_vector(local_position))
        body.set("quat", format_vector(local_quaternion))
        for joint in joints:
            removed.append(joint.attrib["name"])
            body.remove(joint)

    source_key = compiled.key_qpos[0]
    reduced_qpos = []
    removed_set = set(removed)
    widths = {
        int(mujoco.mjtJoint.mjJNT_FREE): 7,
        int(mujoco.mjtJoint.mjJNT_BALL): 4,
        int(mujoco.mjtJoint.mjJNT_SLIDE): 1,
        int(mujoco.mjtJoint.mjJNT_HINGE): 1,
    }
    for joint_id in range(compiled.njnt):
        name = mujoco.mj_id2name(compiled, mujoco.mjtObj.mjOBJ_JOINT, joint_id)
        if name in removed_set:
            continue
        width = widths[int(compiled.jnt_type[joint_id])]
        address = int(compiled.jnt_qposadr[joint_id])
        reduced_qpos.extend(float(value) for value in source_key[address : address + width])
    key = root.find("./keyframe/key")
    if key is None:
        raise ValueError("source model is missing keyframe 0")
    key.set("qpos", format_vector(reduced_qpos))
    return removed


def rewrite_model(
    source_xml: Path,
    output_xml: Path,
    collision: str,
    freeze_passive: bool,
) -> dict[str, object]:
    compiled = mujoco.MjModel.from_xml_path(str(source_xml))
    tree = ET.parse(source_xml)
    root = tree.getroot()
    parents = {child: parent for parent in root.iter() for child in parent}
    elements = {
        element.get("name"): element for element in root.findall(".//geom") if element.get("name")
    }
    proxy_types: Counter[str] = Counter()
    source_geom_names = set(elements)
    proxy_names = []
    if collision == "primitive":
        for geom_id in collidable_fly_meshes(compiled):
            name = mujoco.mj_id2name(compiled, mujoco.mjtObj.mjOBJ_GEOM, geom_id)
            source = elements.get(name)
            if source is None or source.get("type") != "mesh":
                raise ValueError(f"cannot locate source collision mesh {name}")
            parent = parents[source]
            source_index = list(parent).index(source)
            proxy_attributes = build_proxy_attributes(compiled, geom_id, source)
            source.set("name", f"fly/visual/{name.removeprefix('fly/')}")
            source.set("contype", "0")
            source.set("conaffinity", "0")
            proxy = ET.Element("geom", proxy_attributes)
            parent.insert(source_index + 1, proxy)
            proxy_types[proxy.attrib["type"]] += 1
            proxy_names.append(name)
        if len(proxy_names) != 55 or len(set(proxy_names)) != len(proxy_names):
            raise ValueError(f"expected 55 unique collision proxies, generated {len(proxy_names)}")
        if not set(proxy_names).issubset(source_geom_names):
            raise ValueError("collision proxy names do not preserve the source public interface")
    frozen_joints = (
        freeze_fully_passive_bodies(compiled, root, freeze_passive)
        if freeze_passive != "none"
        else []
    )
    ET.indent(tree, space="  ")
    tree.write(output_xml, encoding="utf-8", xml_declaration=False)
    return {
        "source_counts": model_counts(compiled),
        "collision_backend": collision,
        "proxy_count": len(proxy_names),
        "proxy_types": dict(sorted(proxy_types.items())),
        "preserved_collision_geom_names": sorted(proxy_names),
        "frozen_passive_joint_count": len(frozen_joints),
        "frozen_passive_joints": sorted(frozen_joints),
    }


def validate_generated(
    source_xml: Path, output_xml: Path, build: dict[str, object]
) -> dict[str, object]:
    source = mujoco.MjModel.from_xml_path(str(source_xml))
    generated = mujoco.MjModel.from_xml_path(str(output_xml))
    for field in ("nbody", "nu", "nsensor"):
        if int(getattr(source, field)) != int(getattr(generated, field)):
            raise ValueError(f"generated model changed {field}")
    for field in ("body_mass", "body_inertia"):
        left = getattr(source, field)
        right = getattr(generated, field)
        if left.shape != right.shape or any(
            not math.isclose(float(a), float(b), rel_tol=1e-10, abs_tol=1e-14)
            for a, b in zip(left.flat, right.flat)
        ):
            raise ValueError(f"generated model changed {field}")
    generated_collision = fly_collision_types(generated)
    expected_collision = (
        build["proxy_types"]
        if build["collision_backend"] == "primitive"
        else fly_collision_types(source)
    )
    if generated_collision != expected_collision:
        raise ValueError("generated fly collision types do not match the requested backend")

    source_data = mujoco.MjData(source)
    generated_data = mujoco.MjData(generated)
    mujoco.mj_resetDataKeyframe(source, source_data, 0)
    mujoco.mj_resetDataKeyframe(generated, generated_data, 0)
    mujoco.mj_forward(source, source_data)
    mujoco.mj_forward(generated, generated_data)
    maximum_position_error = max(
        math.dist(source_data.xpos[index], generated_data.xpos[index])
        for index in range(source.nbody)
    )
    minimum_quaternion_alignment = min(
        abs(
            sum(
                float(a) * float(b)
                for a, b in zip(source_data.xquat[index], generated_data.xquat[index])
            )
        )
        for index in range(source.nbody)
    )
    if maximum_position_error > 1e-8 or minimum_quaternion_alignment < 1.0 - 1e-10:
        raise ValueError("generated keyframe does not preserve the source body pose")
    return {
        "generated_counts": model_counts(generated),
        "mass_and_inertia_preserved": True,
        "body_actuator_and_sensor_counts_preserved": True,
        "maximum_keyframe_body_position_error": maximum_position_error,
        "minimum_keyframe_body_quaternion_alignment": minimum_quaternion_alignment,
        "generated_fly_collision_types": generated_collision,
    }


def update_manifest(
    path: Path,
    source_manifest: dict,
    build: dict[str, object],
    source_xml: Path,
) -> None:
    model = mujoco.MjModel.from_xml_path(str(path.parent / "fly.xml"))
    counts = model_counts(model)
    source_manifest["counts"].update(
        {key: value for key, value in counts.items() if key != "geoms"}
    )
    source_manifest["neutral_qpos"] = [float(value) for value in model.key_qpos[0]]
    for actuator in source_manifest["actuators"]:
        joint_id = mujoco.mj_name2id(model, mujoco.mjtObj.mjOBJ_JOINT, actuator["joint_name"])
        if joint_id >= 0:
            actuator["joint_index"] = int(joint_id)
    for sensor in source_manifest["sensors"]:
        index = sensor["index"]
        sensor["address"] = int(model.sensor_adr[index])
        sensor["dimension"] = int(model.sensor_dim[index])
        sensor["type"] = int(model.sensor_type[index])
    source_manifest["files"]["fly.xml"] = sha256_file(path.parent / "fly.xml")
    source_manifest["realtime_profile"] = {
        "schema": "flybrain.realtime-assets",
        "schema_version": 1,
        "collision_backend": build["collision_backend"],
        "frozen_passive_joint_count": build["frozen_passive_joint_count"],
        "source_fly_xml_sha256": sha256_file(source_xml),
    }
    path.write_text(json.dumps(source_manifest, indent=2) + "\n", encoding="utf-8")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=Path("assets/neuromechfly"))
    parser.add_argument("--output", type=Path, default=Path("assets/neuromechfly_realtime"))
    parser.add_argument("--collision", choices=("mesh", "primitive"), default="mesh")
    parser.add_argument(
        "--freeze-passive",
        choices=("none", "nonleg", "terminal-feet", "all"),
        default="none",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    source = args.source.resolve()
    output = args.output.resolve()
    if not source.is_dir():
        raise SystemExit(f"source asset directory does not exist: {source}")
    if output.exists():
        raise SystemExit(f"output already exists: {output}")
    if source == output or source in output.parents:
        raise SystemExit("output must be a separate directory outside the source asset tree")
    shutil.copytree(source, output)
    try:
        build = rewrite_model(
            source / "fly.xml",
            output / "fly.xml",
            args.collision,
            args.freeze_passive,
        )
        validation = validate_generated(source / "fly.xml", output / "fly.xml", build)
        source_manifest = json.loads((source / "manifest.json").read_text())
        update_manifest(output / "manifest.json", source_manifest, build, source / "fly.xml")
        report = {
            "schema": "flybrain.realtime-asset-build",
            "schema_version": 1,
            "source": str(args.source),
            "output": str(args.output),
            "source_fly_xml_sha256": sha256_file(source / "fly.xml"),
            "generated_fly_xml_sha256": sha256_file(output / "fly.xml"),
            **build,
            **validation,
        }
        (output / "realtime_asset_manifest.json").write_text(
            json.dumps(report, indent=2) + "\n", encoding="utf-8"
        )
    except Exception:
        shutil.rmtree(output)
        raise
    print(json.dumps(report, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
