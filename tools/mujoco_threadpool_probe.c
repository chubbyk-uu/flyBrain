// Headless single-world diagnostic. Enable only through explicit LD_PRELOAD.
// Do not combine with mujoco_step_probe.c (its collision counters are not thread-safe).
#define _GNU_SOURCE
#include <dlfcn.h>
#include <mujoco/mujoco.h>
#include <stdio.h>
#include <stdlib.h>

static mjData* bound_data;
static mjThreadPool* pool;
static int initialized;

void mj_step(const mjModel* m, mjData* d) {
  static void (*original)(const mjModel*, mjData*);
  if (!original) {
    *(void**)(&original) = dlsym(RTLD_NEXT, "mj_step");
    if (!original) abort();
  }
  if (!initialized) {
    const char* value = getenv("FLYBRAIN_PROBE_WORKERS");
    char* end;
    long workers = value ? strtol(value, &end, 10) : -1;
    if (!value || end == value || *end || workers < 0 || workers > 8 || d->threadpool) {
      fputs("threadpool probe: expected 0..8 workers and unbound data\n", stderr);
      abort();
    }
    bound_data = d;
    if (workers) {
      pool = mju_threadPoolCreate((size_t)workers);
      if (!pool) abort();
      mju_bindThreadPool(d, pool);
    }
    fprintf(stderr, "THREADPROBE workers=%ld nv=%d dt=%.8g version=%s\n",
            workers, (int)m->nv, m->opt.timestep, mj_versionString());
    initialized = 1;
  }
  if (d != bound_data) {
    fputs("threadpool probe supports one simulation data instance only\n", stderr);
    abort();
  }
  original(m, d);
}

void mj_deleteData(mjData* d) {
  static void (*original)(mjData*);
  if (!original) {
    *(void**)(&original) = dlsym(RTLD_NEXT, "mj_deleteData");
    if (!original) abort();
  }
  int owned = d && d == bound_data;
  original(d);
  if (owned) {
    if (pool) mju_threadPoolDestroy(pool);
    pool = NULL;
    bound_data = NULL;
    initialized = 0;
  }
}
