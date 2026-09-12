#include <cuda_runtime.h>

#include <cstddef>
#include <cstdint>
#include <new>
#include <string>
#include <vector>

namespace {

constexpr unsigned int THREADS = 256;
constexpr uint32_t EDGES_PER_TASK = 256;
thread_local std::string last_error;

struct MetricResults {
    uint64_t total_spikes = 0;
    uint32_t active_neurons = 0;
    double voltage_sum = 0.0;
};

struct Engine {
    uint32_t neuron_count = 0;
    uint32_t delay_steps = 0;
    uint32_t ring_size = 1;
    uint64_t step_index = 0;
    float resting_mv = 0.0f;
    float reset_mv = 0.0f;
    float threshold_mv = 0.0f;
    float membrane_decay = 0.0f;
    float synapse_decay = 0.0f;
    float coupling = 0.0f;
    float synapse_weight_mv = 0.0f;
    float external_weight_mv = 0.0f;
    uint32_t* row_ptr = nullptr;
    uint32_t* destinations = nullptr;
    int16_t* signed_counts = nullptr;
    uint32_t* task_sources = nullptr;
    uint32_t* task_starts = nullptr;
    uint32_t* task_ends = nullptr;
    uint32_t task_count = 0;
    bool chunked_propagation = false;
    uint8_t* silenced_sources = nullptr;
    float* voltage = nullptr;
    float* conductance = nullptr;
    int32_t* refractory_remaining = nullptr;
    int32_t* refractory_lengths = nullptr;
    uint8_t* spikes = nullptr;
    uint32_t* spike_counts = nullptr;
    uint8_t* spike_ring = nullptr;
    int32_t* arrivals = nullptr;
    uint32_t* external_targets = nullptr;
    uint32_t external_target_count = 0;
    uint32_t* sparse_lanes = nullptr;
    uint8_t* sparse_counts = nullptr;
    uint32_t sparse_event_capacity = 0;
    uint32_t* probe_indices = nullptr;
    uint32_t* probe_before = nullptr;
    uint32_t* probe_after = nullptr;
    uint32_t probe_capacity = 0;
    MetricResults* metrics = nullptr;
    std::string device_name;

    ~Engine() {
        cudaFree(row_ptr);
        cudaFree(destinations);
        cudaFree(signed_counts);
        cudaFree(task_sources);
        cudaFree(task_starts);
        cudaFree(task_ends);
        cudaFree(silenced_sources);
        cudaFree(voltage);
        cudaFree(conductance);
        cudaFree(refractory_remaining);
        cudaFree(refractory_lengths);
        cudaFree(spikes);
        cudaFree(spike_counts);
        cudaFree(spike_ring);
        cudaFree(arrivals);
        cudaFree(external_targets);
        cudaFree(sparse_lanes);
        cudaFree(sparse_counts);
        cudaFree(probe_indices);
        cudaFree(probe_before);
        cudaFree(probe_after);
        cudaFree(metrics);
    }
};

int fail(const std::string& message) {
    last_error = message;
    return 1;
}

int check(cudaError_t status, const char* operation) {
    if (status == cudaSuccess) {
        return 0;
    }
    return fail(std::string(operation) + ": " + cudaGetErrorString(status));
}

template <typename T>
int allocate_copy(T** destination, const T* source, size_t count, const char* label) {
    if (count == 0) {
        return 0;
    }
    if (check(cudaMalloc(reinterpret_cast<void**>(destination), count * sizeof(T)), label)) {
        return 1;
    }
    return check(cudaMemcpy(*destination, source, count * sizeof(T), cudaMemcpyHostToDevice), label);
}

template <typename T>
int allocate_zero(T** destination, size_t count, const char* label) {
    if (count == 0) {
        return 0;
    }
    if (check(cudaMalloc(reinterpret_cast<void**>(destination), count * sizeof(T)), label)) {
        return 1;
    }
    return check(cudaMemset(*destination, 0, count * sizeof(T)), label);
}

__device__ float updated_voltage(
    float voltage,
    float conductance,
    float resting_mv,
    float membrane_decay,
    float coupling) {
    const float from_rest = __fsub_rn(voltage, resting_mv);
    return __fmaf_rn(coupling, conductance, __fmaf_rn(membrane_decay, from_rest, resting_mv));
}

__global__ void decay_threshold(
    float* voltage,
    float* conductance,
    int32_t* refractory_remaining,
    uint8_t* spikes,
    uint32_t* spike_counts,
    uint32_t neuron_count,
    float resting_mv,
    float threshold_mv,
    float membrane_decay,
    float synapse_decay,
    float coupling) {
    const uint32_t neuron = blockIdx.x * blockDim.x + threadIdx.x;
    if (neuron >= neuron_count) {
        return;
    }
    const int32_t remaining = refractory_remaining[neuron] > 0
        ? refractory_remaining[neuron] - 1
        : 0;
    refractory_remaining[neuron] = remaining;
    const bool can_update = remaining == 0;
    const float old_g = conductance[neuron];
    if (can_update) {
        voltage[neuron] = updated_voltage(
            voltage[neuron], old_g, resting_mv, membrane_decay, coupling);
        conductance[neuron] = __fmul_rn(synapse_decay, old_g);
    }
    const bool fired = can_update && voltage[neuron] > threshold_mv;
    spikes[neuron] = fired ? 1 : 0;
    if (fired) {
        spike_counts[neuron] += 1;
    }
}

__global__ void decay_threshold_propagate_delayed(
    float* voltage,
    float* conductance,
    int32_t* refractory_remaining,
    uint8_t* spikes,
    uint32_t* spike_counts,
    const uint32_t* row_ptr,
    const uint32_t* destinations,
    const int16_t* signed_counts,
    const uint8_t* delayed_spikes,
    const uint8_t* silenced_sources,
    int32_t* arrivals,
    uint32_t neuron_count,
    float resting_mv,
    float threshold_mv,
    float membrane_decay,
    float synapse_decay,
    float coupling) {
    const uint32_t neuron = blockIdx.x * blockDim.x + threadIdx.x;
    if (neuron >= neuron_count) {
        return;
    }
    const int32_t remaining = refractory_remaining[neuron] > 0
        ? refractory_remaining[neuron] - 1
        : 0;
    refractory_remaining[neuron] = remaining;
    const bool can_update = remaining == 0;
    const float old_g = conductance[neuron];
    if (can_update) {
        voltage[neuron] = updated_voltage(
            voltage[neuron], old_g, resting_mv, membrane_decay, coupling);
        conductance[neuron] = __fmul_rn(synapse_decay, old_g);
    }
    const bool fired = can_update && voltage[neuron] > threshold_mv;
    spikes[neuron] = fired ? 1 : 0;
    if (fired) {
        spike_counts[neuron] += 1;
    }
    if (delayed_spikes[neuron] == 0 || silenced_sources[neuron] != 0) {
        return;
    }
    for (uint32_t edge = row_ptr[neuron]; edge < row_ptr[neuron + 1]; ++edge) {
        atomicAdd(&arrivals[destinations[edge]], static_cast<int32_t>(signed_counts[edge]));
    }
}

__global__ void propagate_csr(
    const uint32_t* row_ptr,
    const uint32_t* destinations,
    const int16_t* signed_counts,
    const uint8_t* delayed_spikes,
    const uint8_t* silenced_sources,
    int32_t* arrivals,
    uint32_t neuron_count) {
    const uint32_t source = blockIdx.x * blockDim.x + threadIdx.x;
    if (source >= neuron_count || delayed_spikes[source] == 0 || silenced_sources[source] != 0) {
        return;
    }
    for (uint32_t edge = row_ptr[source]; edge < row_ptr[source + 1]; ++edge) {
        atomicAdd(&arrivals[destinations[edge]], static_cast<int32_t>(signed_counts[edge]));
    }
}

__global__ void propagate_csr_chunked(
    const uint32_t* task_sources,
    const uint32_t* task_starts,
    const uint32_t* task_ends,
    const uint32_t* destinations,
    const int16_t* signed_counts,
    const uint8_t* delayed_spikes,
    const uint8_t* silenced_sources,
    int32_t* arrivals,
    uint32_t task_count) {
    const uint32_t task = blockIdx.x * blockDim.x + threadIdx.x;
    if (task >= task_count) {
        return;
    }
    const uint32_t source = task_sources[task];
    if (delayed_spikes[source] == 0 || silenced_sources[source] != 0) {
        return;
    }
    for (uint32_t edge = task_starts[task]; edge < task_ends[task]; ++edge) {
        atomicAdd(&arrivals[destinations[edge]], static_cast<int32_t>(signed_counts[edge]));
    }
}

__global__ void decay_threshold_propagate_delayed_chunked(
    float* voltage,
    float* conductance,
    int32_t* refractory_remaining,
    uint8_t* spikes,
    uint32_t* spike_counts,
    const uint32_t* task_sources,
    const uint32_t* task_starts,
    const uint32_t* task_ends,
    const uint32_t* destinations,
    const int16_t* signed_counts,
    const uint8_t* delayed_spikes,
    const uint8_t* silenced_sources,
    int32_t* arrivals,
    uint32_t neuron_count,
    uint32_t task_count,
    float resting_mv,
    float threshold_mv,
    float membrane_decay,
    float synapse_decay,
    float coupling) {
    const uint32_t index = blockIdx.x * blockDim.x + threadIdx.x;
    if (index < neuron_count) {
        const int32_t remaining = refractory_remaining[index] > 0
            ? refractory_remaining[index] - 1
            : 0;
        refractory_remaining[index] = remaining;
        const bool can_update = remaining == 0;
        const float old_g = conductance[index];
        if (can_update) {
            voltage[index] = updated_voltage(
                voltage[index], old_g, resting_mv, membrane_decay, coupling);
            conductance[index] = __fmul_rn(synapse_decay, old_g);
        }
        const bool fired = can_update && voltage[index] > threshold_mv;
        spikes[index] = fired ? 1 : 0;
        if (fired) {
            spike_counts[index] += 1;
        }
    }
    if (index < task_count) {
        const uint32_t source = task_sources[index];
        if (delayed_spikes[source] != 0 && silenced_sources[source] == 0) {
            for (uint32_t edge = task_starts[index]; edge < task_ends[index]; ++edge) {
                atomicAdd(&arrivals[destinations[edge]], static_cast<int32_t>(signed_counts[edge]));
            }
        }
    }
}

__global__ void apply_external(
    float* voltage,
    const uint32_t* targets,
    const uint8_t* counts,
    uint32_t offset,
    uint32_t target_count,
    float external_weight_mv) {
    const uint32_t lane = blockIdx.x * blockDim.x + threadIdx.x;
    if (lane >= target_count) {
        return;
    }
    const uint8_t count = counts[offset + lane];
    if (count != 0) {
        const uint32_t target = targets[lane];
        voltage[target] = __fmaf_rn(static_cast<float>(count), external_weight_mv, voltage[target]);
    }
}

__global__ void apply_external_sparse(
    float* voltage,
    const uint32_t* targets_by_lane,
    const uint32_t* lanes,
    const uint8_t* counts,
    uint32_t offset,
    uint32_t event_count,
    float external_weight_mv) {
    const uint32_t event = blockIdx.x * blockDim.x + threadIdx.x;
    if (event >= event_count) {
        return;
    }
    const uint32_t packed = offset + event;
    const uint32_t target = targets_by_lane[lanes[packed]];
    voltage[target] = __fmaf_rn(static_cast<float>(counts[packed]), external_weight_mv, voltage[target]);
}

__global__ void reset_store(
    float* voltage,
    float* conductance,
    int32_t* refractory_remaining,
    const int32_t* refractory_lengths,
    const uint8_t* spikes,
    uint8_t* spike_ring,
    int32_t* arrivals,
    uint32_t neuron_count,
    uint32_t ring_slot,
    float reset_mv,
    float synapse_weight_mv) {
    const uint32_t neuron = blockIdx.x * blockDim.x + threadIdx.x;
    if (neuron >= neuron_count) {
        return;
    }
    const int32_t signed_count = atomicExch(&arrivals[neuron], 0);
    conductance[neuron] = __fmaf_rn(
        static_cast<float>(signed_count), synapse_weight_mv, conductance[neuron]);
    const uint8_t fired = spikes[neuron];
    spike_ring[static_cast<size_t>(ring_slot) * neuron_count + neuron] = fired;
    if (fired != 0) {
        voltage[neuron] = reset_mv;
        conductance[neuron] = 0.0f;
        refractory_remaining[neuron] = refractory_lengths[neuron];
    }
}

__global__ void gather_spike_counts(
    const uint32_t* spike_counts,
    const uint32_t* probe_indices,
    uint32_t* output,
    uint32_t probe_count) {
    const uint32_t probe = blockIdx.x * blockDim.x + threadIdx.x;
    if (probe < probe_count) {
        output[probe] = spike_counts[probe_indices[probe]];
    }
}

__global__ void reduce_metrics(
    const uint32_t* spike_counts,
    const float* voltage,
    uint32_t neuron_count,
    MetricResults* metrics) {
    __shared__ uint64_t spike_sums[THREADS];
    __shared__ uint32_t active_sums[THREADS];
    __shared__ double voltage_sums[THREADS];
    uint64_t spike_sum = 0;
    uint32_t active_sum = 0;
    double voltage_value_sum = 0.0;
    for (uint32_t neuron = threadIdx.x; neuron < neuron_count; neuron += blockDim.x) {
        const uint32_t count = spike_counts[neuron];
        spike_sum += count;
        active_sum += count != 0;
        voltage_value_sum += static_cast<double>(voltage[neuron]);
    }
    spike_sums[threadIdx.x] = spike_sum;
    active_sums[threadIdx.x] = active_sum;
    voltage_sums[threadIdx.x] = voltage_value_sum;
    __syncthreads();
    for (uint32_t stride = blockDim.x / 2; stride != 0; stride /= 2) {
        if (threadIdx.x < stride) {
            spike_sums[threadIdx.x] += spike_sums[threadIdx.x + stride];
            active_sums[threadIdx.x] += active_sums[threadIdx.x + stride];
            voltage_sums[threadIdx.x] += voltage_sums[threadIdx.x + stride];
        }
        __syncthreads();
    }
    if (threadIdx.x == 0) {
        metrics->total_spikes = spike_sums[0];
        metrics->active_neurons = active_sums[0];
        metrics->voltage_sum = voltage_sums[0];
    }
}

int check_launch(const char* operation) {
    return check(cudaPeekAtLastError(), operation);
}

int reserve_probes(Engine* engine, uint32_t probe_count) {
    if (probe_count <= engine->probe_capacity) {
        return 0;
    }
    uint32_t* indices = nullptr;
    uint32_t* before = nullptr;
    uint32_t* after = nullptr;
    if (check(cudaMalloc(reinterpret_cast<void**>(&indices), probe_count * sizeof(uint32_t)),
              "allocate probe indices")) goto failure;
    if (check(cudaMalloc(reinterpret_cast<void**>(&before), probe_count * sizeof(uint32_t)),
              "allocate probe before counts")) goto failure;
    if (check(cudaMalloc(reinterpret_cast<void**>(&after), probe_count * sizeof(uint32_t)),
              "allocate probe after counts")) goto failure;
    cudaFree(engine->probe_indices);
    cudaFree(engine->probe_before);
    cudaFree(engine->probe_after);
    engine->probe_indices = indices;
    engine->probe_before = before;
    engine->probe_after = after;
    engine->probe_capacity = probe_count;
    return 0;

failure:
    cudaFree(indices);
    cudaFree(before);
    cudaFree(after);
    return 1;
}

int reserve_sparse_events(Engine* engine, uint32_t event_count) {
    if (event_count <= engine->sparse_event_capacity) {
        return 0;
    }
    uint32_t* lanes = nullptr;
    uint8_t* counts = nullptr;
    if (check(cudaMalloc(reinterpret_cast<void**>(&lanes), event_count * sizeof(uint32_t)),
              "allocate sparse lanes")) goto failure;
    if (check(cudaMalloc(reinterpret_cast<void**>(&counts), event_count * sizeof(uint8_t)),
              "allocate sparse counts")) goto failure;
    cudaFree(engine->sparse_lanes);
    cudaFree(engine->sparse_counts);
    engine->sparse_lanes = lanes;
    engine->sparse_counts = counts;
    engine->sparse_event_capacity = event_count;
    return 0;

failure:
    cudaFree(lanes);
    cudaFree(counts);
    return 1;
}

int gather_probes(Engine* engine, uint32_t* output, uint32_t probe_count, const char* operation) {
    if (probe_count == 0) {
        return 0;
    }
    const unsigned int blocks = (probe_count + THREADS - 1) / THREADS;
    gather_spike_counts<<<blocks, THREADS>>>(
        engine->spike_counts, engine->probe_indices, output, probe_count);
    return check_launch(operation);
}

int launch_tick_dense(
    Engine* engine,
    const uint32_t* targets,
    const uint8_t* counts,
    uint32_t target_count,
    uint32_t event_step) {
    const unsigned int blocks = (engine->neuron_count + THREADS - 1) / THREADS;
    const uint32_t ring_slot = engine->step_index % engine->ring_size;
    if (engine->delay_steps == 0) {
        decay_threshold<<<blocks, THREADS>>>(
            engine->voltage, engine->conductance, engine->refractory_remaining,
            engine->spikes, engine->spike_counts, engine->neuron_count,
            engine->resting_mv, engine->threshold_mv, engine->membrane_decay,
            engine->synapse_decay, engine->coupling);
        if (check_launch("launch decay_threshold")) return 1;
        if (engine->chunked_propagation && engine->task_count != 0) {
            const unsigned int task_blocks = (engine->task_count + THREADS - 1) / THREADS;
            propagate_csr_chunked<<<task_blocks, THREADS>>>(
                engine->task_sources, engine->task_starts, engine->task_ends,
                engine->destinations, engine->signed_counts, engine->spikes,
                engine->silenced_sources, engine->arrivals, engine->task_count);
            if (check_launch("launch propagate_csr_chunked")) return 1;
        } else {
            propagate_csr<<<blocks, THREADS>>>(
                engine->row_ptr, engine->destinations, engine->signed_counts,
                engine->spikes, engine->silenced_sources, engine->arrivals,
                engine->neuron_count);
            if (check_launch("launch propagate_csr")) return 1;
        }
    } else {
        const uint8_t* delayed = engine->spike_ring
            + static_cast<size_t>(ring_slot) * engine->neuron_count;
        if (engine->chunked_propagation) {
            const uint32_t work_count = engine->task_count > engine->neuron_count
                ? engine->task_count
                : engine->neuron_count;
            const unsigned int work_blocks = (work_count + THREADS - 1) / THREADS;
            decay_threshold_propagate_delayed_chunked<<<work_blocks, THREADS>>>(
                engine->voltage, engine->conductance, engine->refractory_remaining,
                engine->spikes, engine->spike_counts, engine->task_sources,
                engine->task_starts, engine->task_ends, engine->destinations,
                engine->signed_counts, delayed, engine->silenced_sources,
                engine->arrivals, engine->neuron_count, engine->task_count,
                engine->resting_mv, engine->threshold_mv, engine->membrane_decay,
                engine->synapse_decay, engine->coupling);
            if (check_launch("launch decay_threshold_propagate_delayed_chunked")) return 1;
        } else {
            decay_threshold_propagate_delayed<<<blocks, THREADS>>>(
                engine->voltage, engine->conductance, engine->refractory_remaining,
                engine->spikes, engine->spike_counts, engine->row_ptr,
                engine->destinations, engine->signed_counts, delayed,
                engine->silenced_sources, engine->arrivals, engine->neuron_count,
                engine->resting_mv, engine->threshold_mv, engine->membrane_decay,
                engine->synapse_decay, engine->coupling);
            if (check_launch("launch decay_threshold_propagate_delayed")) return 1;
        }
    }
    if (target_count != 0) {
        const unsigned int event_blocks = (target_count + THREADS - 1) / THREADS;
        apply_external<<<event_blocks, THREADS>>>(
            engine->voltage, targets, counts, event_step * target_count,
            target_count, engine->external_weight_mv);
        if (check_launch("launch apply_external")) return 1;
    }
    reset_store<<<blocks, THREADS>>>(
        engine->voltage, engine->conductance, engine->refractory_remaining,
        engine->refractory_lengths, engine->spikes, engine->spike_ring,
        engine->arrivals, engine->neuron_count, ring_slot, engine->reset_mv,
        engine->synapse_weight_mv);
    if (check_launch("launch reset_store")) return 1;
    engine->step_index += 1;
    return 0;
}

int launch_tick_sparse(
    Engine* engine,
    const uint32_t* targets,
    const uint32_t* lanes,
    const uint8_t* counts,
    uint32_t offset,
    uint32_t event_count) {
    const unsigned int blocks = (engine->neuron_count + THREADS - 1) / THREADS;
    const uint32_t ring_slot = engine->step_index % engine->ring_size;
    if (engine->delay_steps == 0) {
        decay_threshold<<<blocks, THREADS>>>(
            engine->voltage, engine->conductance, engine->refractory_remaining,
            engine->spikes, engine->spike_counts, engine->neuron_count,
            engine->resting_mv, engine->threshold_mv, engine->membrane_decay,
            engine->synapse_decay, engine->coupling);
        if (check_launch("launch decay_threshold")) return 1;
        if (engine->chunked_propagation && engine->task_count != 0) {
            const unsigned int task_blocks = (engine->task_count + THREADS - 1) / THREADS;
            propagate_csr_chunked<<<task_blocks, THREADS>>>(
                engine->task_sources, engine->task_starts, engine->task_ends,
                engine->destinations, engine->signed_counts, engine->spikes,
                engine->silenced_sources, engine->arrivals, engine->task_count);
            if (check_launch("launch propagate_csr_chunked")) return 1;
        } else {
            propagate_csr<<<blocks, THREADS>>>(
                engine->row_ptr, engine->destinations, engine->signed_counts,
                engine->spikes, engine->silenced_sources, engine->arrivals,
                engine->neuron_count);
            if (check_launch("launch propagate_csr")) return 1;
        }
    } else {
        const uint8_t* delayed = engine->spike_ring
            + static_cast<size_t>(ring_slot) * engine->neuron_count;
        if (engine->chunked_propagation) {
            const uint32_t work_count = engine->task_count > engine->neuron_count
                ? engine->task_count
                : engine->neuron_count;
            const unsigned int work_blocks = (work_count + THREADS - 1) / THREADS;
            decay_threshold_propagate_delayed_chunked<<<work_blocks, THREADS>>>(
                engine->voltage, engine->conductance, engine->refractory_remaining,
                engine->spikes, engine->spike_counts, engine->task_sources,
                engine->task_starts, engine->task_ends, engine->destinations,
                engine->signed_counts, delayed, engine->silenced_sources,
                engine->arrivals, engine->neuron_count, engine->task_count,
                engine->resting_mv, engine->threshold_mv, engine->membrane_decay,
                engine->synapse_decay, engine->coupling);
            if (check_launch("launch decay_threshold_propagate_delayed_chunked")) return 1;
        } else {
            decay_threshold_propagate_delayed<<<blocks, THREADS>>>(
                engine->voltage, engine->conductance, engine->refractory_remaining,
                engine->spikes, engine->spike_counts, engine->row_ptr,
                engine->destinations, engine->signed_counts, delayed,
                engine->silenced_sources, engine->arrivals, engine->neuron_count,
                engine->resting_mv, engine->threshold_mv, engine->membrane_decay,
                engine->synapse_decay, engine->coupling);
            if (check_launch("launch decay_threshold_propagate_delayed")) return 1;
        }
    }
    if (event_count != 0) {
        const unsigned int event_blocks = (event_count + THREADS - 1) / THREADS;
        apply_external_sparse<<<event_blocks, THREADS>>>(
            engine->voltage, targets, lanes, counts, offset, event_count,
            engine->external_weight_mv);
        if (check_launch("launch apply_external_sparse")) return 1;
    }
    reset_store<<<blocks, THREADS>>>(
        engine->voltage, engine->conductance, engine->refractory_remaining,
        engine->refractory_lengths, engine->spikes, engine->spike_ring,
        engine->arrivals, engine->neuron_count, ring_slot, engine->reset_mv,
        engine->synapse_weight_mv);
    if (check_launch("launch reset_store")) return 1;
    engine->step_index += 1;
    return 0;
}

}  // namespace

extern "C" const char* flybrain_cuda_last_error() {
    return last_error.c_str();
}

extern "C" int flybrain_cuda_create(
    void** output,
    const uint32_t* row_ptr,
    size_t row_ptr_count,
    const uint32_t* destinations,
    const int16_t* signed_counts,
    size_t edge_count,
    const uint8_t* silenced_sources,
    const float* initial_voltage,
    const float* initial_conductance,
    const int32_t* refractory_lengths,
    const uint32_t* external_targets,
    uint32_t external_target_count,
    uint8_t chunked_propagation,
    uint32_t neuron_count,
    uint32_t delay_steps,
    float resting_mv,
    float reset_mv,
    float threshold_mv,
    float membrane_decay,
    float synapse_decay,
    float coupling,
    float synapse_weight_mv,
    float external_weight_mv) {
    last_error.clear();
    if (output == nullptr || neuron_count == 0 || row_ptr_count != static_cast<size_t>(neuron_count) + 1) {
        return fail("invalid CUDA engine dimensions");
    }
    Engine* engine = new (std::nothrow) Engine();
    if (engine == nullptr) {
        return fail("allocating CUDA engine host state failed");
    }
    engine->neuron_count = neuron_count;
    engine->delay_steps = delay_steps;
    engine->ring_size = delay_steps == 0 ? 1 : delay_steps;
    engine->resting_mv = resting_mv;
    engine->reset_mv = reset_mv;
    engine->threshold_mv = threshold_mv;
    engine->membrane_decay = membrane_decay;
    engine->synapse_decay = synapse_decay;
    engine->coupling = coupling;
    engine->synapse_weight_mv = synapse_weight_mv;
    engine->external_weight_mv = external_weight_mv;
    engine->external_target_count = external_target_count;
    engine->chunked_propagation = chunked_propagation != 0;

    std::vector<uint32_t> task_sources;
    std::vector<uint32_t> task_starts;
    std::vector<uint32_t> task_ends;
    if (engine->chunked_propagation) {
        task_sources.reserve(edge_count / EDGES_PER_TASK + neuron_count);
        task_starts.reserve(edge_count / EDGES_PER_TASK + neuron_count);
        task_ends.reserve(edge_count / EDGES_PER_TASK + neuron_count);
        for (uint32_t source = 0; source < neuron_count; ++source) {
            const uint32_t end = row_ptr[source + 1];
            for (uint32_t start = row_ptr[source]; start < end; start += EDGES_PER_TASK) {
                task_sources.push_back(source);
                task_starts.push_back(start);
                const uint32_t remaining = end - start;
                task_ends.push_back(start + (remaining < EDGES_PER_TASK ? remaining : EDGES_PER_TASK));
            }
        }
        engine->task_count = static_cast<uint32_t>(task_sources.size());
    }

    cudaDeviceProp properties{};
    if (check(cudaSetDevice(0), "select CUDA device")) goto failure;
    if (check(cudaGetDeviceProperties(&properties, 0), "query CUDA device")) goto failure;
    engine->device_name = properties.name;
    if (allocate_copy(&engine->row_ptr, row_ptr, row_ptr_count, "upload row_ptr")) goto failure;
    if (allocate_copy(&engine->destinations, destinations, edge_count, "upload destinations")) goto failure;
    if (allocate_copy(&engine->signed_counts, signed_counts, edge_count, "upload signed_counts")) goto failure;
    if (allocate_copy(&engine->task_sources, task_sources.data(), task_sources.size(),
                      "upload propagation task sources")) goto failure;
    if (allocate_copy(&engine->task_starts, task_starts.data(), task_starts.size(),
                      "upload propagation task starts")) goto failure;
    if (allocate_copy(&engine->task_ends, task_ends.data(), task_ends.size(),
                      "upload propagation task ends")) goto failure;
    if (allocate_copy(&engine->silenced_sources, silenced_sources, neuron_count, "upload silenced_sources")) goto failure;
    if (allocate_copy(&engine->voltage, initial_voltage, neuron_count, "upload voltage")) goto failure;
    if (allocate_copy(&engine->conductance, initial_conductance, neuron_count, "upload conductance")) goto failure;
    if (allocate_zero(&engine->refractory_remaining, neuron_count, "allocate refractory_remaining")) goto failure;
    if (allocate_copy(&engine->refractory_lengths, refractory_lengths, neuron_count, "upload refractory_lengths")) goto failure;
    if (allocate_zero(&engine->spikes, neuron_count, "allocate spikes")) goto failure;
    if (allocate_zero(&engine->spike_counts, neuron_count, "allocate spike_counts")) goto failure;
    if (allocate_zero(
            &engine->spike_ring,
            static_cast<size_t>(engine->ring_size) * neuron_count,
            "allocate spike_ring")) goto failure;
    if (allocate_zero(&engine->arrivals, neuron_count, "allocate arrivals")) goto failure;
    if (allocate_copy(&engine->external_targets, external_targets, external_target_count,
                      "upload external targets")) goto failure;
    if (allocate_zero(&engine->metrics, 1, "allocate metric results")) goto failure;
    *output = engine;
    return 0;

failure:
    delete engine;
    return 1;
}

extern "C" void flybrain_cuda_destroy(void* raw_engine) {
    delete static_cast<Engine*>(raw_engine);
}

extern "C" const char* flybrain_cuda_device_name(void* raw_engine) {
    return static_cast<Engine*>(raw_engine)->device_name.c_str();
}

extern "C" int flybrain_cuda_run_dense(
    void* raw_engine,
    const uint32_t* host_targets,
    uint32_t target_count,
    const uint8_t* host_counts,
    uint32_t steps,
    uint32_t chunk_steps) {
    last_error.clear();
    Engine* engine = static_cast<Engine*>(raw_engine);
    uint32_t* targets = nullptr;
    uint8_t* counts = nullptr;
    const size_t count_length = static_cast<size_t>(steps) * target_count;
    if (allocate_copy(&targets, host_targets, target_count, "upload dense targets")) goto failure;
    if (allocate_copy(&counts, host_counts, count_length, "upload dense counts")) goto failure;
    for (uint32_t step = 0; step < steps; ++step) {
        if (launch_tick_dense(engine, targets, counts, target_count, step)) goto failure;
        if ((step + 1) % chunk_steps == 0 || step + 1 == steps) {
            if (check(cudaDeviceSynchronize(), "synchronize dense schedule")) goto failure;
        }
    }
    cudaFree(targets);
    cudaFree(counts);
    return 0;

failure:
    cudaFree(targets);
    cudaFree(counts);
    return 1;
}

extern "C" int flybrain_cuda_run_sparse(
    void* raw_engine,
    const uint32_t* host_targets,
    uint32_t target_count,
    const uint32_t* host_offsets,
    const uint32_t* host_lanes,
    const uint8_t* host_counts,
    uint32_t event_count,
    uint32_t steps) {
    last_error.clear();
    Engine* engine = static_cast<Engine*>(raw_engine);
    uint32_t* targets = nullptr;
    uint32_t* lanes = nullptr;
    uint8_t* counts = nullptr;
    if (allocate_copy(&targets, host_targets, target_count, "upload sparse targets")) goto failure;
    if (allocate_copy(&lanes, host_lanes, event_count, "upload sparse lanes")) goto failure;
    if (allocate_copy(&counts, host_counts, event_count, "upload sparse counts")) goto failure;
    for (uint32_t step = 0; step < steps; ++step) {
        const uint32_t offset = host_offsets[step];
        const uint32_t step_event_count = host_offsets[step + 1] - offset;
        if (launch_tick_sparse(engine, targets, lanes, counts, offset, step_event_count)) goto failure;
    }
    if (check(cudaDeviceSynchronize(), "synchronize sparse window")) goto failure;
    cudaFree(targets);
    cudaFree(lanes);
    cudaFree(counts);
    return 0;

failure:
    cudaFree(targets);
    cudaFree(lanes);
    cudaFree(counts);
    return 1;
}

extern "C" int flybrain_cuda_run_sparse_probed(
    void* raw_engine,
    const uint32_t* host_offsets,
    const uint32_t* host_lanes,
    const uint8_t* host_counts,
    uint32_t event_count,
    uint32_t steps,
    const uint32_t* host_probes,
    uint32_t probe_count,
    uint8_t refresh_probes,
    uint32_t* host_before,
    uint32_t* host_after) {
    last_error.clear();
    Engine* engine = static_cast<Engine*>(raw_engine);
    if (refresh_probes && reserve_probes(engine, probe_count)) goto failure;
    if (reserve_sparse_events(engine, event_count)) goto failure;
    if (refresh_probes && probe_count != 0) {
        if (check(cudaMemcpy(engine->probe_indices, host_probes,
                             probe_count * sizeof(uint32_t), cudaMemcpyHostToDevice),
                  "upload probe indices")) goto failure;
        if (gather_probes(engine, engine->probe_before, probe_count,
                          "launch gather probe counts before window")) goto failure;
    }
    if (event_count != 0) {
        if (check(cudaMemcpy(engine->sparse_lanes, host_lanes,
                             event_count * sizeof(uint32_t), cudaMemcpyHostToDevice),
                  "upload sparse lanes")) goto failure;
        if (check(cudaMemcpy(engine->sparse_counts, host_counts,
                             event_count * sizeof(uint8_t), cudaMemcpyHostToDevice),
                  "upload sparse counts")) goto failure;
    }
    for (uint32_t step = 0; step < steps; ++step) {
        const uint32_t offset = host_offsets[step];
        const uint32_t step_event_count = host_offsets[step + 1] - offset;
        if (launch_tick_sparse(engine, engine->external_targets, engine->sparse_lanes,
                               engine->sparse_counts, offset, step_event_count)) goto failure;
    }
    if (gather_probes(engine, engine->probe_after, probe_count,
                      "launch gather probe counts after window")) goto failure;
    if (refresh_probes && probe_count != 0) {
        if (check(cudaMemcpy(host_before, engine->probe_before,
                             probe_count * sizeof(uint32_t), cudaMemcpyDeviceToHost),
                  "read probe counts before window")) goto failure;
    }
    if (probe_count != 0) {
        if (check(cudaMemcpy(host_after, engine->probe_after,
                             probe_count * sizeof(uint32_t), cudaMemcpyDeviceToHost),
                  "read probe counts after window")) goto failure;
    } else if (check(cudaDeviceSynchronize(), "synchronize sparse probed window")) {
        goto failure;
    }
    return 0;

failure:
    return 1;
}

extern "C" int flybrain_cuda_read_metrics(
    void* raw_engine,
    uint64_t* host_total_spikes,
    uint32_t* host_active_neurons,
    double* host_voltage_sum) {
    last_error.clear();
    Engine* engine = static_cast<Engine*>(raw_engine);
    reduce_metrics<<<1, THREADS>>>(
        engine->spike_counts, engine->voltage, engine->neuron_count,
        engine->metrics);
    if (check_launch("launch CUDA metrics reduction")) return 1;
    MetricResults metrics{};
    if (check(cudaMemcpy(&metrics, engine->metrics, sizeof(metrics), cudaMemcpyDeviceToHost),
              "read CUDA metrics")) return 1;
    *host_total_spikes = metrics.total_spikes;
    *host_active_neurons = metrics.active_neurons;
    *host_voltage_sum = metrics.voltage_sum;
    return 0;
}

extern "C" int flybrain_cuda_copy_state(
    void* raw_engine,
    uint8_t* spikes,
    uint32_t* spike_counts,
    float* voltage,
    float* conductance) {
    last_error.clear();
    Engine* engine = static_cast<Engine*>(raw_engine);
    const size_t count = engine->neuron_count;
    if (check(cudaMemcpy(spikes, engine->spikes, count * sizeof(uint8_t), cudaMemcpyDeviceToHost), "read spikes")) return 1;
    if (check(cudaMemcpy(spike_counts, engine->spike_counts, count * sizeof(uint32_t), cudaMemcpyDeviceToHost), "read spike_counts")) return 1;
    if (check(cudaMemcpy(voltage, engine->voltage, count * sizeof(float), cudaMemcpyDeviceToHost), "read voltage")) return 1;
    if (check(cudaMemcpy(conductance, engine->conductance, count * sizeof(float), cudaMemcpyDeviceToHost), "read conductance")) return 1;
    return 0;
}
