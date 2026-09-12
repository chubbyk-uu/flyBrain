# 本机环境检查与恢复记录 — 2026-09-12

本记录区分实际执行结果与尚未验证的目标；项目范围见 [总规范](project-spec.md)。

## 共享环境恢复

最初检查发现 `rlpy312` 使用 Python 3.12.13、NumPy 2.4.6、MuJoCo 3.9.0、
PyTorch 2.12.0。一次项目准备操作新增了 `flybrain-engine` 可编辑安装、PyArrow 25.0.1
和 Brian2 2.10.1，并因 Brian2 的依赖约束将 Cython 3.2.5 降为 3.1.3。

按用户要求，已卸载上述三个新增包，将 Cython 恢复为 3.2.5。
包元数据复查确认三个新增包已不存在、Cython 已恢复，NumPy/MuJoCo/PyTorch 版本
保持原值，`pip check` 通过。未删除环境或修改其他 Conda 环境。
这是对已知包改动的撤销，不声称清除所有下载缓存或逐字节还原整个环境。

## 独立环境

已创建 `flybrain`，Python 3.12.14。安装版本和命令见 [local setup](local-setup.md)。
创建和安装进程继承当前 shell 的 `HTTP_PROXY`、`HTTPS_PROXY` 和 `NO_PROXY`；
未修改全局代理设置，本文不保存私人代理地址或凭据。

独立环境复验结果：

- `python -m pip check`：通过，无损坏依赖。
- `python -m pytest -q tests/test_male_cns.py tests/test_reference.py tests/test_brian_parity.py tests/test_verify_cns_world.py`：
  29 项通过，耗时 45.88 秒；27 条 Brian2 内部 Pyparsing 弃用警告，无失败或跳过。
- MuJoCo 3.9.0 与 GLFW 导入成功；独立最小球体模型执行一步，仿真时间变为 0.002 秒。
- CUDA Driver API 初始化与设备查询成功，返回 1 张显卡。未执行 CUDA 神经内核。

文档本地链接检查及 `git diff --check` 通过。仓库改动仅为 README 与三份说明文档，
未修改仿真源码、模型参数、数据或映射。

## 系统工具及已有证据

| 检查项 | 观测结果 |
|---|---|
| 操作系统 | x86_64 WSL2 |
| GPU | RTX 5080，约 16 GB 显存；`nvidia-smi` 可识别 |
| CUDA Toolkit | `nvcc` 13.0.88；NVRTC 动态库存在 |
| CUDA Driver API | `cuInit(0)` 成功，设备查询返回 1 张显卡 |
| Rust | 1.96.0 |
| Node | 24.16.0 |
| CMake | 4.2.3 |
| 浏览器资源 | 已提交 WASM；49 个运行资源的 SHA-256 校验通过 |
| MaleCNS 数据 | 本地 pack 缺失；线上 manifest 声明的四个数组哈希与现有 I/O 相符 |

已检查线上部分文件/首分块的 HEAD 请求成功，尚未完整下载、重组或校验全部数组。
系统 CUDA 的设备初始化不等于 CUDA 神经内核执行成功；后端尚未实现。
完整 CNS WebGPU 浏览器仿真、Linux 原生链接和 WSLg 显示尚未验收。

先前在共享环境的临时安装中，29 项针对 MaleCNS 导入、参考模拟、Brian2 对照和
CNS world 报告验证的测试通过；MuJoCo 最小模型步进成功。这些结果不能代替
新独立环境的复验，也不是完整 CNS 或 GPU 一致性验收。
