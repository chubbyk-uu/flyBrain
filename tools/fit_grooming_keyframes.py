"""Offline, read-only kinematic fit of engineering grooming keyframes.

No model mutation is saved. The resulting commands still require actual dynamics
and visual acceptance; inverse kinematics alone is not a successful action.
"""
import json
from pathlib import Path
import mujoco
import numpy as np

root = Path(__file__).resolve().parents[1]
model = mujoco.MjModel.from_xml_path(str(root / 'assets/neuromechfly/indoor-v2.xml'))
data = mujoco.MjData(model)
neutral = np.array(json.loads((root / 'assets/neuromechfly/manifest.json').read_text())['neutral_control'])
for actuator in range(42):
    joint = model.actuator_trnid[actuator, 0]
    data.qpos[model.jnt_qposadr[joint]] = neutral[actuator]
data.qpos[:3] = [0, 0, 2]
data.qpos[3:7] = [1, 0, 0, 0]
mujoco.mj_forward(model, data)
initial = data.qpos.copy()
foot = mujoco.mj_name2id(model, mujoco.mjtObj.mjOBJ_BODY, 'fly/lf_tarsus5')
eye = mujoco.mj_name2id(model, mujoco.mjtObj.mjOBJ_GEOM, 'fly/l_eye')
mesh = model.geom_dataid[eye]
vertices = model.mesh_vert[model.mesh_vertadr[mesh]:model.mesh_vertadr[mesh]+model.mesh_vertnum[mesh]]
world_vertices = vertices @ data.geom_xmat[eye].reshape(3,3).T + data.geom_xpos[eye]
target_eye = (world_vertices.min(axis=0)+world_vertices.max(axis=0))/2
target_eye[1] = world_vertices[:,1].max()
joints = model.actuator_trnid[:7, 0]
qadr = model.jnt_qposadr[joints]
dadr = model.jnt_dofadr[joints]
result = {}
for name, target in [('rub_near', target_eye + [0.75, -target_eye[1]+0.025, -0.05]),
                     ('rub_far', target_eye + [0.95, -target_eye[1]+0.025, -0.05]),
                     ('original_rub_reference', target_eye + [0.25, -target_eye[1]+0.07, -0.15]),
                     ('head', target_eye + [0.02, 0.08, 0.02])]:
    if name != 'head':
        data.qpos[:] = initial
    for _ in range(250):
        mujoco.mj_forward(model, data)
        error = target-data.xpos[foot]
        jac = np.zeros((3, model.nv))
        mujoco.mj_jacBody(model, data, jac, None, foot)
        j = jac[:, dadr]
        delta = j.T @ np.linalg.solve(j @ j.T + np.eye(3)*0.003, error)
        data.qpos[qadr] += np.clip(delta, -0.08, 0.08)
        # Compensation for the unchanged joint springs and actuator gain.
        commands = data.qpos[qadr] + model.jnt_stiffness[joints]/50*(data.qpos[qadr]-model.qpos_spring[qadr])
        commands = np.clip(commands, -3.14, 3.14)
        data.qpos[qadr] = (commands+model.jnt_stiffness[joints]/50*model.qpos_spring[qadr])/(1+model.jnt_stiffness[joints]/50)
    mujoco.mj_forward(model, data)
    result[name] = dict(target=target.tolist(), achieved=data.xpos[foot].tolist(),
                        command_offset=(commands-neutral[:7]).tolist(),
                        error_mm=float(np.linalg.norm(target-data.xpos[foot])))
print(json.dumps(result, indent=2))
