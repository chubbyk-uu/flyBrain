// Optional LD_PRELOAD diagnostics, single CUDA simulation thread only.
// GPU event span covers the neural graph, not individual kernel active time.
#include <cuda_runtime_api.h>
#include <dlfcn.h>
#include <cstdio>
#include <cstdlib>
#include <ctime>

static double seconds() {
  timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return t.tv_sec + t.tv_nsec * 1e-9;
}
template <typename T> static T symbol(const char* name) {
  void* p = dlsym(RTLD_NEXT, name);
  if (!p) std::abort();
  return reinterpret_cast<T>(p);
}
static unsigned long graphs, measured, stream_syncs, device_syncs;
static unsigned long calls[5], bytes[5], max_bytes[5];
static double copy_wall[5], sync_wall, device_wall, graph_ms;
static cudaEvent_t start_event, end_event;
static cudaStream_t timed_stream;
static bool pending;

extern "C" cudaError_t cudaGraphLaunch(cudaGraphExec_t graph, cudaStream_t stream) {
  static auto original = symbol<decltype(&cudaGraphLaunch)>("cudaGraphLaunch");
  if (!graphs) {
    if (cudaEventCreate(&start_event) != cudaSuccess ||
        cudaEventCreate(&end_event) != cudaSuccess) std::abort();
  }
  if (pending) std::abort();  // current engine synchronizes each window
  if (cudaEventRecord(start_event, stream) != cudaSuccess) std::abort();
  auto result = original(graph, stream);
  if (cudaEventRecord(end_event, stream) != cudaSuccess) std::abort();
  timed_stream = stream;
  pending = true;
  ++graphs;
  return result;
}

extern "C" cudaError_t cudaMemcpy(void* dst, const void* src, size_t count, cudaMemcpyKind kind) {
  static auto original = symbol<decltype(&cudaMemcpy)>("cudaMemcpy");
  double began = seconds();
  auto result = original(dst, src, count, kind);
  if (graphs && kind >= 0 && kind < 5) {
    ++calls[kind];
    bytes[kind] += count;
    if (count > max_bytes[kind]) max_bytes[kind] = count;
    copy_wall[kind] += seconds() - began;
  }
  return result;
}

extern "C" cudaError_t cudaStreamSynchronize(cudaStream_t stream) {
  static auto original = symbol<decltype(&cudaStreamSynchronize)>("cudaStreamSynchronize");
  double began = seconds();
  auto result = original(stream);
  if (graphs) {
    ++stream_syncs;
    sync_wall += seconds() - began;
  }
  if (pending && stream == timed_stream && result == cudaSuccess) {
    float elapsed;
    if (cudaEventElapsedTime(&elapsed, start_event, end_event) != cudaSuccess) std::abort();
    graph_ms += elapsed;
    ++measured;
    pending = false;
  }
  return result;
}

extern "C" cudaError_t cudaDeviceSynchronize() {
  static auto original = symbol<decltype(&cudaDeviceSynchronize)>("cudaDeviceSynchronize");
  double began = seconds();
  auto result = original();
  if (graphs) { ++device_syncs; device_wall += seconds() - began; }
  return result;
}

__attribute__((destructor)) static void report() {
  std::fprintf(stderr,
      "CUDABENCH {\"graphs\":%lu,\"measured_graphs\":%lu,\"gpu_graph_seconds\":%.9f,"
      "\"stream_sync_calls\":%lu,\"stream_sync_seconds\":%.9f,"
      "\"device_sync_calls\":%lu,\"device_sync_seconds\":%.9f,"
      "\"h2d_calls\":%lu,\"h2d_bytes\":%lu,\"h2d_max_bytes\":%lu,\"h2d_wall_seconds\":%.9f,"
      "\"d2h_calls\":%lu,\"d2h_bytes\":%lu,\"d2h_max_bytes\":%lu,\"d2h_wall_seconds\":%.9f}\n",
      graphs, measured, graph_ms / 1000., stream_syncs, sync_wall, device_syncs, device_wall,
      calls[1], bytes[1], max_bytes[1], copy_wall[1],
      calls[2], bytes[2], max_bytes[2], copy_wall[2]);
  // Driver teardown owns remaining event allocations at process exit.
}
