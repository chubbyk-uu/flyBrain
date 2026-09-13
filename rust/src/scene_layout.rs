use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use mujoco_rs::prelude::{MjSpec, MjtGeom, SpecItem};
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub const LEGACY_SCENE_ID: &str = "legacy";

#[derive(Clone, Debug, PartialEq)]
pub struct SceneMetadata {
    pub spawn_position_mm: Option<[f64; 3]>,
    pub id: String,
    pub schema: String,
    pub sha256: String,
    pub source: PathBuf,
    pub habitat_file: String,
    pub room_half_extents_mm: [f64; 3],
    pub food_center_mm: [f64; 3],
    pub active_geoms: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SceneLayout {
    #[serde(default)]
    spawn_position_mm: Option<[f64; 3]>,
    #[serde(default)]
    pub model_file: Option<String>,
    schema: String,
    id: String,
    habitat_file: String,
    room_half_extents_mm: [f64; 3],
    food_center_mm: [f64; 3],
    #[serde(default)]
    disabled_geoms: Vec<String>,
    #[serde(default)]
    overrides: Vec<GeomOverride>,
    #[serde(default)]
    additions: Vec<GeomAddition>,
}

#[derive(Debug, Deserialize)]
struct GeomOverride {
    name: String,
    pos: [f64; 3],
    size: [f64; 3],
}

#[derive(Debug, Deserialize)]
struct GeomAddition {
    name: String,
    shape: GeomShape,
    pos: [f64; 3],
    size: [f64; 3],
    #[serde(default)]
    material: String,
    #[serde(default = "default_rgba")]
    rgba: [f32; 4],
    #[serde(default)]
    collidable: bool,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum GeomShape {
    Box,
    Sphere,
    Cylinder,
    Ellipsoid,
}

fn default_rgba() -> [f32; 4] {
    [1.0; 4]
}

impl SceneLayout {
    pub fn resolve(assets_dir: &Path, selection: &str) -> Result<Option<PathBuf>> {
        if selection == LEGACY_SCENE_ID {
            return Ok(None);
        }
        let direct = Path::new(selection);
        let path = if direct.components().count() > 1 || direct.extension().is_some() {
            direct.to_path_buf()
        } else {
            assets_dir.join("scenes").join(format!("{selection}.json"))
        };
        if !path.is_file() {
            bail!("scene layout does not exist: {}", path.display())
        }
        Ok(Some(path))
    }

    pub fn load(path: impl AsRef<Path>) -> Result<(Self, SceneMetadata)> {
        let path = path.as_ref();
        let bytes =
            fs::read(path).with_context(|| format!("reading scene layout {}", path.display()))?;
        let layout: Self = serde_json::from_slice(&bytes)
            .with_context(|| format!("parsing scene layout {}", path.display()))?;
        layout.validate()?;
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        let active_geoms = layout
            .overrides
            .iter()
            .map(|geom| geom.name.clone())
            .chain(layout.additions.iter().map(|geom| geom.name.clone()))
            .collect();
        let metadata = SceneMetadata {
            spawn_position_mm: layout.spawn_position_mm,
            id: layout.id.clone(),
            schema: layout.schema.clone(),
            sha256,
            source: path.to_path_buf(),
            habitat_file: layout.habitat_file.clone(),
            room_half_extents_mm: layout.room_half_extents_mm,
            food_center_mm: layout.food_center_mm,
            active_geoms,
        };
        Ok((layout, metadata))
    }

    fn validate(&self) -> Result<()> {
        if self.schema != "flybrain-scene-layout-v1" || self.id.is_empty() {
            bail!("unsupported scene layout schema or empty id")
        }
        if self.model_file.as_ref().is_some_and(|name| {
            let path = Path::new(name);
            path.components().count() != 1
                || path.extension().and_then(|s| s.to_str()) != Some("xml")
        }) || self.spawn_position_mm.is_some_and(|p| {
            p.iter().any(|v| !v.is_finite())
                || p[0].abs() >= self.room_half_extents_mm[0]
                || p[1].abs() >= self.room_half_extents_mm[1]
                || p[2] <= 0.0
                || p[2] >= 2.0 * self.room_half_extents_mm[2]
        }) {
            bail!("scene model filename or spawn position is invalid")
        }
        if self.habitat_file.is_empty()
            || self
                .room_half_extents_mm
                .iter()
                .any(|value| !value.is_finite() || *value <= 0.0)
            || self.food_center_mm.iter().any(|value| !value.is_finite())
        {
            bail!("scene layout has invalid habitat or room dimensions")
        }
        let mut names = std::collections::BTreeSet::new();
        for name in &self.disabled_geoms {
            if name.is_empty() || !names.insert(name.as_str()) {
                bail!("scene layout contains an empty or duplicate geom name")
            }
        }
        for geom in &self.overrides {
            validate_geom(&geom.name, geom.pos, geom.size)?;
            if !names.insert(geom.name.as_str()) {
                bail!("scene layout contains duplicate geom {}", geom.name)
            }
        }
        for geom in &self.additions {
            validate_geom(&geom.name, geom.pos, geom.size)?;
            if !names.insert(geom.name.as_str())
                || geom.rgba.iter().any(|value| !value.is_finite())
                || geom.material.contains('\0')
            {
                bail!("scene layout contains invalid added geom {}", geom.name)
            }
        }
        Ok(())
    }

    pub fn apply(&self, spec: &mut MjSpec) -> Result<()> {
        for name in &self.disabled_geoms {
            let geom = spec
                .geom_mut(name)
                .ok_or_else(|| anyhow::anyhow!("scene disables missing geom {name}"))?;
            geom.with_rgba([0.0, 0.0, 0.0, 0.0]);
            geom.set_contype(0);
            geom.set_conaffinity(0);
            geom.set_group(3);
        }
        for replacement in &self.overrides {
            let geom = spec.geom_mut(&replacement.name).ok_or_else(|| {
                anyhow::anyhow!("scene overrides missing geom {}", replacement.name)
            })?;
            geom.with_pos(replacement.pos).with_size(replacement.size);
        }
        for addition in &self.additions {
            if spec.geom(&addition.name).is_some() {
                bail!("scene adds duplicate geom {}", addition.name)
            }
            let geom = spec
                .world_body_mut()
                .add_geom()
                .with_name(&addition.name)
                .with_pos(addition.pos)
                .with_size(addition.size)
                .with_rgba(addition.rgba);
            geom.set_type(match addition.shape {
                GeomShape::Box => MjtGeom::mjGEOM_BOX,
                GeomShape::Sphere => MjtGeom::mjGEOM_SPHERE,
                GeomShape::Cylinder => MjtGeom::mjGEOM_CYLINDER,
                GeomShape::Ellipsoid => MjtGeom::mjGEOM_ELLIPSOID,
            });
            if !addition.material.is_empty() {
                geom.set_material(&addition.material);
            }
            geom.set_mass(0.0);
            geom.set_contype(0);
            geom.set_conaffinity(i32::from(addition.collidable));
            if addition.collidable {
                geom.set_condim(3);
                geom.set_priority(1);
                geom.with_friction([1.0, 0.02, 0.0001]);
            }
        }
        Ok(())
    }
}

fn validate_geom(name: &str, pos: [f64; 3], size: [f64; 3]) -> Result<()> {
    if name.is_empty()
        || name.contains('\0')
        || pos.iter().any(|value| !value.is_finite())
        || size.iter().any(|value| !value.is_finite() || *value <= 0.0)
    {
        bail!("scene layout geom {name:?} is invalid")
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_legacy_without_an_overlay() {
        assert_eq!(
            SceneLayout::resolve(Path::new("assets"), "legacy").unwrap(),
            None
        );
    }

    #[test]
    fn rejects_duplicate_geometry_roles() {
        let layout: SceneLayout = serde_json::from_str(
            r#"{"schema":"flybrain-scene-layout-v1","id":"x","habitat_file":"x.json","room_half_extents_mm":[1,1,1],"food_center_mm":[0,0,0],"disabled_geoms":["same"],"overrides":[{"name":"same","pos":[0,0,0],"size":[1,1,1]}]}"#,
        )
        .unwrap();
        assert!(layout.validate().is_err());
    }
}
