use mujoco_rs::prelude::*;
use serde_json::{Value, json};

pub fn scene_descriptor(
    model: &MjModel,
    neurons: usize,
    brain_model: Option<&str>,
    backend: Option<&str>,
) -> Value {
    let name = |kind, id| model.id_to_name(kind, id).unwrap_or("").to_string();
    let meshes: Vec<_> = (0..model.nmesh() as usize)
        .map(|i| {
            let v = model.mesh_vertadr()[i] as usize;
            let f = model.mesh_faceadr()[i] as usize;
            let n = model.mesh_normaladr()[i] as usize;
            json!({
                "vertices": &model.mesh_vert()[v..v + model.mesh_vertnum()[i] as usize],
                "faces": &model.mesh_face()[f..f + model.mesh_facenum()[i] as usize],
                "normals": &model.mesh_normal()[n..n + model.mesh_normalnum()[i] as usize],
                "faceNormals": &model.mesh_facenormal()[f..f + model.mesh_facenum()[i] as usize],
            })
        })
        .collect();
    let geoms: Vec<_> = (0..model.ngeom() as usize)
        .map(|i| {
            let material = model.geom_matid()[i];
            let mut color = model.geom_rgba()[i];
            let mut repeat = [1.0_f32; 2];
            if material >= 0 {
                color = model.mat_rgba()[material as usize];
                repeat = model.mat_texrepeat()[material as usize];
                let texture =
                    model.mat_texid()[material as usize][MjtTextureRole::mjTEXROLE_RGB as usize];
                if texture >= 0 {
                    let t = texture as usize;
                    let start = model.tex_adr()[t] as usize;
                    let pixels = model.tex_width()[t] as usize * model.tex_height()[t] as usize;
                    let channels = model.tex_nchannel()[t] as usize;
                    let bytes = &model.tex_data()[start..start + pixels * channels];
                    let mut sums = [0_u64; 3];
                    for pixel in bytes.chunks_exact(channels) {
                        for channel in 0..3 {
                            sums[channel] += u64::from(pixel[channel.min(channels - 1)]);
                        }
                    }
                    let material_name = name(MjtObj::mjOBJ_MATERIAL, material as usize);
                    if !matches!(
                        material_name.as_str(),
                        "grid" | "habitat/wood" | "habitat/darkwood"
                    ) {
                        for channel in 0..3 {
                            color[channel] *= sums[channel] as f32 / (pixels as f32 * 255.0);
                        }
                    }
                }
            }
            json!({
                "id": i,
                "name": name(MjtObj::mjOBJ_GEOM, i),
                "body": model.geom_bodyid()[i],
                "type": model.geom_type()[i] as i32,
                "size": model.geom_size()[i],
                "pos": model.geom_pos()[i],
                "quat": model.geom_quat()[i],
                "mesh": model.geom_dataid()[i],
                "rgba": color,
                "material": if material >= 0 {
                    name(MjtObj::mjOBJ_MATERIAL, material as usize)
                } else {
                    String::new()
                },
                "group": model.geom_group()[i],
                "texrepeat": repeat,
            })
        })
        .collect();
    let cameras: Vec<_> = (0..model.ncam() as usize)
        .map(|i| {
            json!({
                "name": name(MjtObj::mjOBJ_CAMERA, i),
                "body": model.cam_bodyid()[i],
                "pos": model.cam_pos()[i],
                "quat": model.cam_quat()[i],
                "fovy": model.cam_fovy()[i],
            })
        })
        .collect();
    json!({
        "bodyCount": model.nbody(),
        "meshCount": model.nmesh(),
        "geoms": geoms,
        "meshes": meshes,
        "cameras": cameras,
        "brain": {"neurons": neurons, "model": brain_model, "backend": backend},
    })
}

pub fn body_poses<M>(data: &MjData<M>) -> Vec<f32>
where
    M: std::ops::Deref<Target = MjModel>,
{
    let mut poses = Vec::with_capacity(data.xpos().len() * 7 + data.cam_xpos().len() * 12);
    for (position, quaternion) in data.xpos().iter().zip(data.xquat()) {
        poses.extend(position.iter().chain(quaternion).map(|value| *value as f32));
    }
    for (position, rotation) in data.cam_xpos().iter().zip(data.cam_xmat()) {
        poses.extend(position.iter().chain(rotation).map(|value| *value as f32));
    }
    poses
}
