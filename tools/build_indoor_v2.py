"""Build a standalone room from an empty worldbody and the unchanged fly subtree.

No legacy environmental geometry/material is imported. All positions below are mm.
Generated XML lives beside fly.xml so unchanged mesh paths remain valid.
"""
from copy import deepcopy
import hashlib
import json
from pathlib import Path
import math
import xml.etree.ElementTree as ET

ASSETS = Path(__file__).resolve().parents[1] / "assets/neuromechfly"
SUGAR = [48.0, -12.0, 30.6]
NECTAR = [-48.0, 12.0, 65.4]
SPAWN = [26.0, -12.0, 32.1]


def build():
    source = ET.parse(ASSETS / "fly.xml").getroot()
    root = ET.Element("mujoco", model="indoor-v2")
    for element in source:
        if element.tag not in {"asset", "worldbody"}:
            root.append(deepcopy(element))
    root.find("option").set("wind", "0 0 0")
    asset = ET.SubElement(root, "asset")
    for item in source.find("asset"):
        if item.get("name", "").startswith("fly/"):
            asset.append(deepcopy(item))
    colors = {
        "plaster": "0.82 0.80 0.74 1", "floor": "0.57 0.61 0.58 1",
        "ashwood": "0.62 0.43 0.25 1", "ceramic": "0.26 0.43 0.43 1",
        "soil": "0.12 0.085 0.05 1", "leaf": "0.18 0.38 0.12 1",
        "petal": "0.92 0.64 0.72 1", "nectar": "0.83 0.52 0.12 1",
        "candy": "0.90 0.30 0.12 1",
    }
    for name, rgba in colors.items():
        ET.SubElement(asset, "material", name=f"indoor-v2/{name}", rgba=rgba,
                      specular="0.15" if name in {"ceramic", "candy"} else "0.04",
                      shininess="0.15")
    world = ET.SubElement(root, "worldbody")
    fly = deepcopy(source.find("worldbody/body[@name='fly/c_thorax']"))
    world.append(fly)
    ET.SubElement(world, "site", name="fly", pos="0 0 0.8")
    ET.SubElement(world, "camera", name="room_camera", pos="220 -290 220",
                  xyaxes="0.797 0.604 0 -0.30 0.396 0.868", fovy="52")
    ET.SubElement(world, "light", pos="0 -50 135", dir="0 0 -1", directional="true",
                  diffuse="0.7 0.68 0.63", castshadow="true")

    def geom(name, shape, pos, size, material, collision=True, **extra):
        return ET.SubElement(world, "geom", name=name, type=shape,
            pos=" ".join(map(str, pos)), size=" ".join(map(str, size)),
            material=f"indoor-v2/{material}", mass="0", contype="0",
            conaffinity="1" if collision else "0", condim="3", priority="1",
            friction="1 0.02 0.0001", solref="0.0002",
            solimp="0.98 0.99 1e-05 0.5 3", margin="0.001", **extra)

    # The fly's explicit ground contact pairs remain authoritative.
    geom("ground_plane", "plane", [0, 0, 0], [150, 110, 1], "floor", False)
    for name, pos, size in [
        ("left", [-152, 0, 70], [2, 110, 70]),
        ("right", [152, 0, 70], [2, 110, 70]),
        ("back", [0, 112, 70], [150, 2, 70]),
        ("front_window", [0, -112, 70], [150, 2, 70]),
        ("ceiling", [0, 0, 142], [150, 110, 2]),
    ]:
        geom(f"room_wall_{name}", "box", pos, size, "plaster")
    geom("table_top", "box", [0, 0, 28], [90, 40, 2], "ashwood")
    for i, (x, y) in enumerate([(-78, -29), (78, -29), (-78, 29), (78, 29)]):
        geom(f"table_leg_{i}", "box", [x, y, 13], [3, 3, 13], "ashwood")
    # One exposed low candy surface, not several decorative false food targets.
    geom("food_patch", "ellipsoid", SUGAR, [3.5, 3, 0.6], "candy", False)
    geom("plant_pot", "cylinder", [-48, 12, 36], [9, 6], "ceramic")
    geom("plant_soil", "cylinder", [-48, 12, 42.05], [8.1, 0.15], "soil")
    geom("plant_stem", "cylinder", [-48, 12, 53.2], [0.65, 11.2], "leaf", False)
    for i, (z, side) in enumerate([(47, -1), (51, 1), (55, -1), (59, 1)]):
        tilt = side * -0.25
        geom(f"plant_leaf_{i}", "ellipsoid", [-48 + side * 4.5, 12, z + 1.1],
             [5.1, 2.2, 0.25], "leaf", False,
             quat=f"{math.cos(tilt/2)} 0 {math.sin(tilt/2)} 0")
    # A thin contiguous landing calyx; petal collision is its documented approximation.
    geom("flower_support", "cylinder", [-48, 12, 64.4], [7.5, 0.6], "leaf")
    for i in range(8):
        a = i * math.tau / 8
        geom(f"flower_petal_{i}", "ellipsoid",
             [-48 + 5 * math.cos(a), 12 + 5 * math.sin(a), 65], [5, 2.5, 0.55],
             "petal", False, quat=f"{math.cos(a/2)} 0 0 {math.sin(a/2)}")
    geom("resource_nectar", "cylinder", NECTAR, [2.8, 0.4], "nectar", False)
    for i in range(13):
        a = i * 2.4
        radius = 0.55 * math.sqrt(i)
        geom(f"flower_stamen_{i}", "sphere",
             [-48 + radius * math.cos(a), 12 + radius * math.sin(a), 65.87],
             [0.25], "nectar", False)
    # Ground contact pairs and all fly dynamics are preserved byte-semantically.
    fly_hash = hashlib.sha256(ET.tostring(fly)).hexdigest()
    ET.indent(root)
    ET.ElementTree(root).write(ASSETS / "indoor-v2.xml", encoding="unicode")
    resources = []
    for name, geom_name, pos in [("sugar_drop", "food_patch", SUGAR),
                                 ("flower_nectar", "resource_nectar", NECTAR)]:
        resources.append(dict(id=name, kind="food", geom=geom_name, position=pos,
            movable=name == "sugar_drop", taste_radius_mm=3.0, odor_source_ppm=175.0,
            odor_length_mm=35.0, taste_valence=1.0, nutrition=1.0, hydration=0.3))
    habitat = dict(schema="flybrain-habitat-v2", room=dict(half_extents_mm=[150,110,70],
        open_ceiling=False, front_doorway_width_mm=0, flight_altitude_bounds_mm=[5,110]),
        airflow_mm_s=[0,0,0], resources=resources)
    habitat["units"] = dict(length="millimeter", time="second", mass="gram",
                            odor_concentration="isobutylene-equivalent ppm")
    habitat["sensory_model"] = dict(odor="advection-diffusion plume v1",
        taste="mouth-to-resource distance v2", vision="native binocular FlyGym retina")
    layout = dict(schema="flybrain-scene-layout-v1", id="indoor-v2",
        model_file="indoor-v2.xml", habitat_file="indoor-v2-habitat.json",
        room_half_extents_mm=[150,110,70], food_center_mm=SUGAR,
        spawn_position_mm=SPAWN, fly_subtree_sha256=fly_hash)
    for filename, data in [("indoor-v2.json", layout), ("indoor-v2-habitat.json", habitat)]:
        (ASSETS / "scenes" / filename).write_text(json.dumps(data, indent=2) + "\n")


if __name__ == "__main__":
    build()
