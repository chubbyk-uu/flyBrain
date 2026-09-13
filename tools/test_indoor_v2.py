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
            vertical_radius = size[1] if geom.get("type") == "cylinder" else radius
            self.assertLess(pos[2] + vertical_radius, 140)
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

    def test_detached_petals_have_physical_collision(self):
        names = [f"flower_petal_{i}" for i in range(8)]
        for name in names:
            geom = self.world.find(f"geom[@name='{name}']")
            self.assertEqual(geom.get("conaffinity"), "1", name)

    def test_flat_flower_and_thin_sugar_rest_on_table(self):
        for geom in self.world.findall("geom"):
            self.assertFalse(geom.get("name").startswith("plant_"))
        self.assertIsNone(self.world.find("geom[@name='flower_corolla']"))
        self.assertIsNone(self.world.find("geom[@name='flower_support']"))
        sugar = self.world.find("geom[@name='food_patch']")
        self.assertAlmostEqual(float(sugar.get("size").split()[2]), 0.2)
        self.assertAlmostEqual(SUGAR[2] - 0.2, 30)
        nectar = self.world.find("geom[@name='resource_nectar']")
        self.assertAlmostEqual(float(nectar.get("size").split()[1]), 0.2)
        self.assertAlmostEqual(NECTAR[2] - 0.2, 30)
        for i in range(8):
            petal = self.world.find(f"geom[@name='flower_petal_{i}']")
            z = float(petal.get("pos").split()[2])
            half = float(petal.get("size").split()[2])
            self.assertAlmostEqual(half, 0.01)
            self.assertAlmostEqual(z - half, 30)
            self.assertLess(z + half, NECTAR[2])

    def test_ceiling_light_is_inside_room(self):
        light = self.world.find("light[@name='ceiling_light']")
        self.assertEqual(light.get("castshadow"), "true")
        self.assertEqual(light.get("dir"), "0 0 -1")
        self.assertLess(float(light.get("pos").split()[2]), 140)


if __name__ == "__main__":
    unittest.main()
