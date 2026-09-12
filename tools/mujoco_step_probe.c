// Headless, single-thread diagnostic only. LD_PRELOAD this library to measure
// MuJoCo's nested pipeline timers without changing the model or production code.
// Compile/link instructions and limitations are in the accompanying report.
#define _GNU_SOURCE
#include <dlfcn.h>
#include <mujoco/mujoco.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static double seconds(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return (double)t.tv_sec + (double)t.tv_nsec * 1e-9;
}

static mjfCollision original_collision[mjNGEOMTYPES][mjNGEOMTYPES];
static double* pair_seconds;
static unsigned long* pair_calls;

static int collision_probe(const mjModel* m, mjData* d, mjPreContact* con,
                           int g1, int g2, mjtNum margin) {
  double started = seconds();
  int count = original_collision[m->geom_type[g1]][m->geom_type[g2]](
      m, d, con, g1, g2, margin);
  size_t index = (size_t)g1 * m->ngeom + g2;
  pair_seconds[index] += seconds() - started;
  ++pair_calls[index];
  return count;
}

void mj_step(const mjModel* m, mjData* d) {
  static void (*original)(const mjModel*, mjData*);
  static double totals[mjNTIMER];
  static int steps, max_contacts, max_constraints;
  static double next_report = 1.0;
  if (!original) {
    *(void**)(&original) = dlsym(RTLD_NEXT, "mj_step");
    mjfTime* timer = (mjfTime*)dlsym(RTLD_NEXT, "mjcb_time");
    if (!original || !timer || *timer) {
      fputs("mujoco_step_probe: missing symbols or existing timer callback\n", stderr);
      abort();
    }
    *timer = seconds;
    const char* collision = getenv("FLYBRAIN_PROFILE_COLLISION");
    if (collision && strcmp(collision, "1") == 0) {
      size_t n = (size_t)m->ngeom * m->ngeom;
      pair_seconds = calloc(n, sizeof(*pair_seconds));
      pair_calls = calloc(n, sizeof(*pair_calls));
      if (!pair_seconds || !pair_calls) abort();
      memcpy(original_collision, mjCOLLISIONFUNC, sizeof(original_collision));
      for (int i = 0; i < mjNGEOMTYPES; ++i)
        for (int j = 0; j < mjNGEOMTYPES; ++j)
          if (original_collision[i][j]) mjCOLLISIONFUNC[i][j] = collision_probe;
    }
    fprintf(stderr, "MJPROBE model nv=%d ngeom=%d jacobian=%d solver=%d "
                    "integrator=%d iterations=%d noslip=%d dt=%.8g\n",
            (int)m->nv, (int)m->ngeom, m->opt.jacobian, m->opt.solver,
            m->opt.integrator, m->opt.iterations, m->opt.noslip_iterations,
            m->opt.timestep);
  }
  double before[mjNTIMER];
  for (int i = 0; i < mjNTIMER; ++i) before[i] = d->timer[i].duration;
  original(m, d);
  for (int i = 0; i < mjNTIMER; ++i)
    totals[i] += d->timer[i].duration - before[i];
  ++steps;
  if (d->ncon > max_contacts) max_contacts = d->ncon;
  if (d->nefc > max_constraints) max_constraints = d->nefc;
  if (d->time + 1e-8 >= next_report) {
    fprintf(stderr, "MJPROBE t=%.3f steps=%d max_ncon=%d max_nefc=%d",
            d->time, steps, max_contacts, max_constraints);
    for (int i = 0; i < mjNTIMER; ++i)
      fprintf(stderr, " %s=%.6f", mjTIMERSTRING[i], totals[i]);
    fputc('\n', stderr);
    if (pair_seconds) {
      size_t best = 0;
      double total = 0;
      unsigned long calls = 0;
      for (size_t i = 0; i < (size_t)m->ngeom * m->ngeom; ++i) {
        if (pair_seconds[i] > pair_seconds[best]) best = i;
        total += pair_seconds[i];
        calls += pair_calls[i];
      }
      fprintf(stderr, "MJPAIR total=%.6f calls=%lu worst=%s/%s time=%.6f calls=%lu\n",
              total, calls, mj_id2name(m, mjOBJ_GEOM, best / m->ngeom),
              mj_id2name(m, mjOBJ_GEOM, best % m->ngeom),
              pair_seconds[best], pair_calls[best]);
    }
    next_report += 1.0;
  }
}
