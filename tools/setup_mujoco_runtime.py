from __future__ import annotations

import glob
import os
import sys
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]
VENV = PROJECT_ROOT / "work/upstream/flygym/.venv"
RUNTIME = PROJECT_ROOT / "work/mujoco/lib"


def find_macos_library() -> Path:
    candidates = glob.glob(
        str(VENV / "lib/python*/site-packages/mujoco/libmujoco.3.9.0.dylib")
    )
    if len(candidates) != 1:
        raise RuntimeError(
            f"expected one MuJoCo 3.9.0 library under {VENV}, found {len(candidates)}"
        )
    return Path(candidates[0]).resolve()


def find_macos_glfw() -> Path:
    candidates = glob.glob(str(VENV / "lib/python*/site-packages/glfw/libglfw.3.dylib"))
    if len(candidates) != 1:
        raise RuntimeError(
            f"expected one GLFW library under {VENV}, found {len(candidates)}"
        )
    return Path(candidates[0]).resolve()


def replace_symlink(path: Path, target: Path) -> None:
    if path.is_symlink():
        path.unlink()
    elif path.exists():
        raise FileExistsError(f"refusing to replace non-symlink: {path}")
    path.symlink_to(os.path.relpath(target, path.parent))


def main() -> int:
    RUNTIME.mkdir(parents=True, exist_ok=True)
    if sys.platform == "linux":
        import mujoco

        library = Path(mujoco.__file__).resolve().parent / "libmujoco.so.3.9.0"
        if not library.is_file():
            raise RuntimeError(f"MuJoCo 3.9.0 library not found: {library}")
        glfw_candidates = glob.glob(
            str(Path(mujoco.__file__).resolve().parent.parent / "glfw/x11/libglfw.so")
        )
        if len(glfw_candidates) != 1:
            raise RuntimeError(
                f"expected one Linux X11 GLFW library, found {len(glfw_candidates)}"
            )
        glfw = Path(glfw_candidates[0]).resolve()
        replace_symlink(RUNTIME / "libmujoco.so", library)
        replace_symlink(RUNTIME / "libmujoco.so.3.9.0", library)
        replace_symlink(RUNTIME / "libglfw.so", glfw)
        replace_symlink(RUNTIME / "libglfw.so.3", glfw)
        replace_symlink(RUNTIME / "libglfw.3.so", glfw)
    elif sys.platform == "darwin":
        library = find_macos_library()
        glfw = find_macos_glfw()
        framework = RUNTIME / "mujoco.framework/Versions/A"
        framework.mkdir(parents=True, exist_ok=True)
        replace_symlink(RUNTIME / "libmujoco.dylib", library)
        replace_symlink(RUNTIME / "libglfw.3.dylib", glfw)
        replace_symlink(framework / "libmujoco.3.9.0.dylib", library)
    else:
        raise RuntimeError(f"unsupported native MuJoCo platform: {sys.platform}")
    print(RUNTIME)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
