"""Static, reproducible scene-contract checks; does not replace visual inspection."""
import json
import unittest
import xml.etree.ElementTree as ET
from build_indoor_v2 import ASSETS, SUGAR, NECTAR, SPAWN


def structural(element):
    return element.tag, sorted(element.attrib.items()), [structural(child) for child in element]


class IndoorContract(unittest.TestCase):
    def setUp(self):
        self.root = ET.parse(ASSETS / "indoor-v2.xml").getroot()
        self.world = self.root.find("worldbody")

    def test_unchanged_fly_dynamics(self):
        original = ET.parse(ASSETS / "fly.xml").getroot()
        for selector in ["worldbody/body[@name='fly/c_thorax']", "actuator", "sensor", "contact", "keyframe"]:
            self.assertEqual(structural(self.root.find(selector)), structural(original.find(selector)), selector)

    def test_environment_is_self_contained(self):
        self.assertEqual(len(self.world.findall("body")), 1)
        for geom in self.world.findall("geom"):
            name = geom.get("name")
            self.assertTrue(geom.get("material").startswith("indoor-v2/"), name)
            self.assertFalse(name.startswith("detail_") or "rug" in name, name)
            if name.startswith("room_wall_") or name == "ground_plane":
                continue
            pos = list(map(float, geom.get("pos").split()))
            size = list(map(float, geom.get("size").split()))
            # Conservative enclosing sphere for rotated leaf/petal bounds.
            radius = max(size)
            self.assertLess(abs(pos[0]) + radius, 150)
            self.assertLess(abs(pos[1]) + radius, 110)
            self.assertLess(pos[2] + radius, 140)
        top = self.world.find("geom[@name='table_top']")
        self.assertEqual(top.get("size"), "90 40 2")
        for leg in self.world.findall("geom"):
            if leg.get("name").startswith("table_leg_"):
                self.assertEqual(float(leg.get("pos").split()[2]) + float(leg.get("size").split()[2]), 26)

    def test_resources_and_spawn(self):
        habitat = json.loads((ASSETS / "scenes/indoor-v2-habitat.json").read_text())
        self.assertEqual(habitat["airflow_mm_s"], [0, 0, 0])
        for resource, expected in zip(habitat["resources"], [SUGAR, NECTAR]):
            geom = self.world.find(f"geom[@name='{resource['geom']}']")
            self.assertEqual(list(map(float, geom.get("pos").split())), expected)
            self.assertEqual(resource["position"], expected)
        distance = sum((a-b)**2 for a,b in zip(SPAWN, SUGAR))**0.5
        self.assertTrue(15 <= distance <= 35)
        self.assertGreater(distance, 3 + 5 + 1)  # includes conservative mouth offset


if __name__ == "__main__":
    unittest.main()
