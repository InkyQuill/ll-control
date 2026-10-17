/* Compile-only UAPI check; never opens a GPU. See docs/development/parity/intel-memory-query.md. */
#include <stddef.h>
#include <stdio.h>
#include "i915_drm.h"
#include "xe_drm.h"
_Static_assert(sizeof(struct drm_i915_query) == 16, "i915 query");
_Static_assert(sizeof(struct drm_i915_query_item) == 24, "i915 item");
_Static_assert(sizeof(struct drm_xe_device_query) == 40, "xe query");
_Static_assert(sizeof(struct drm_i915_memory_region_info) == 88, "i915 region");
_Static_assert(sizeof(struct drm_xe_mem_region) == 88, "xe region");
_Static_assert(offsetof(struct drm_i915_query_memory_regions,regions) == 16, "i915 header");
_Static_assert(offsetof(struct drm_xe_query_mem_regions,mem_regions) == 8, "xe header");
_Static_assert(offsetof(struct drm_i915_memory_region_info,probed_size) == 8, "i915 total");
_Static_assert(offsetof(struct drm_i915_memory_region_info,unallocated_size) == 16, "i915 free");
_Static_assert(offsetof(struct drm_xe_mem_region,total_size) == 8, "xe total");
_Static_assert(offsetof(struct drm_xe_mem_region,used) == 16, "xe used");
_Static_assert(sizeof(struct drm_xe_engine) == 32, "xe engine");
_Static_assert(offsetof(struct drm_xe_query_engines,engines) == 8, "xe engines header");
_Static_assert(offsetof(struct drm_xe_engine_class_instance,gt_id) == 4, "xe GT id");
int main(void) {printf("version=%lx i915=%lx xe=%lx\n", (unsigned long)DRM_IOCTL_VERSION, (unsigned long)DRM_IOCTL_I915_QUERY, (unsigned long)DRM_IOCTL_XE_DEVICE_QUERY);}
