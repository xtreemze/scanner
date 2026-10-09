#include <jni.h>
#include <dlfcn.h>
#include <stdint.h>
#include <stddef.h>
#include <string.h>

#include "scanner_core.h"

typedef ScannerNativeIngestSession *(*session_create_fn)(const uint8_t *, size_t, uint64_t);
typedef void (*session_destroy_fn)(ScannerNativeIngestSession *);
typedef ScannerNativeStatus (*ingest_camera_fn)(
    ScannerNativeIngestSession *, const ScannerNativeCameraFrameMetadata *);
typedef ScannerNativeStatus (*ingest_imu_fn)(
    ScannerNativeIngestSession *, const ScannerNativeImuSample *);
typedef ScannerNativeStatus (*ingest_depth_fn)(
    ScannerNativeIngestSession *, const ScannerNativeDepthDescriptor *,
    const uint8_t *, size_t, const uint8_t *, size_t);
typedef ScannerNativeStatus (*ingest_stats_fn)(
    ScannerNativeIngestSession *, ScannerNativeIngestStats *);

typedef struct ScannerCoreSymbols {
  session_create_fn create;
  session_destroy_fn destroy;
  ingest_camera_fn camera;
  ingest_imu_fn imu;
  ingest_depth_fn depth;
  ingest_stats_fn stats;
  int initialized;
} ScannerCoreSymbols;

static ScannerCoreSymbols g_symbols = {0};

static void *scanner_symbol(const char *name) {
  void *symbol = dlsym(RTLD_DEFAULT, name);
  if (symbol != NULL) {
    return symbol;
  }

  const char *candidates[] = {
      "libscanner_core.so",
      "libscanner_lib.so",
      "libscanner_app.so",
  };

  for (size_t i = 0; i < sizeof(candidates) / sizeof(candidates[0]); ++i) {
    void *handle = dlopen(candidates[i], RTLD_NOW | RTLD_GLOBAL);
    if (handle == NULL) {
      continue;
    }
    symbol = dlsym(handle, name);
    if (symbol != NULL) {
      return symbol;
    }
  }

  return NULL;
}

static int ensure_symbols(void) {
  if (g_symbols.initialized) {
    return g_symbols.create != NULL &&
           g_symbols.destroy != NULL &&
           g_symbols.camera != NULL &&
           g_symbols.imu != NULL &&
           g_symbols.depth != NULL &&
           g_symbols.stats != NULL;
  }

  g_symbols.initialized = 1;
  g_symbols.create = (session_create_fn)scanner_symbol("scanner_native_session_create");
  g_symbols.destroy = (session_destroy_fn)scanner_symbol("scanner_native_session_destroy");
  g_symbols.camera =
      (ingest_camera_fn)scanner_symbol("scanner_native_ingest_camera_metadata");
  g_symbols.imu = (ingest_imu_fn)scanner_symbol("scanner_native_ingest_imu");
  g_symbols.depth = (ingest_depth_fn)scanner_symbol("scanner_native_ingest_depth");
  g_symbols.stats = (ingest_stats_fn)scanner_symbol("scanner_native_ingest_stats");

  return g_symbols.create != NULL &&
         g_symbols.destroy != NULL &&
         g_symbols.camera != NULL &&
         g_symbols.imu != NULL &&
         g_symbols.depth != NULL &&
         g_symbols.stats != NULL;
}

JNIEXPORT jlong JNICALL
Java_com_xtreemze_scanner_sensors_ScannerCoreIngestBridge_nativeCreate(
    JNIEnv *env, jclass clazz, jstring device_id, jlong epoch) {
  (void)clazz;
  if (!ensure_symbols() || device_id == NULL || epoch < 0) {
    return 0;
  }

  const char *chars = (*env)->GetStringUTFChars(env, device_id, NULL);
  if (chars == NULL) {
    return 0;
  }

  size_t len = strlen(chars);
  ScannerNativeIngestSession *session =
      g_symbols.create((const uint8_t *)chars, len, (uint64_t)epoch);
  (*env)->ReleaseStringUTFChars(env, device_id, chars);

  return (jlong)(uintptr_t)session;
}

JNIEXPORT void JNICALL
Java_com_xtreemze_scanner_sensors_ScannerCoreIngestBridge_nativeDestroy(
    JNIEnv *env, jclass clazz, jlong handle) {
  (void)env;
  (void)clazz;
  if (!ensure_symbols() || handle == 0) {
    return;
  }
  g_symbols.destroy((ScannerNativeIngestSession *)(uintptr_t)handle);
}

JNIEXPORT jint JNICALL
Java_com_xtreemze_scanner_sensors_ScannerCoreIngestBridge_nativeIngestCamera(
    JNIEnv *env,
    jclass clazz,
    jlong handle,
    jlong timestamp_micros,
    jint clock_domain,
    jint uncertainty_micros,
    jdouble px,
    jdouble py,
    jdouble pz,
    jdouble qx,
    jdouble qy,
    jdouble qz,
    jdouble qw,
    jint width_px,
    jint height_px,
    jdouble fx,
    jdouble fy,
    jdouble cx,
    jdouble cy) {
  (void)env;
  (void)clazz;
  if (!ensure_symbols() || handle == 0) {
    return SCANNER_NATIVE_NULL_HANDLE;
  }
  if (timestamp_micros < 0 || uncertainty_micros < 0 ||
      width_px <= 0 || height_px <= 0) {
    return SCANNER_NATIVE_INVALID_METADATA;
  }

  ScannerNativeCameraFrameMetadata metadata = {0};
  metadata.timestamp_micros = (uint64_t)timestamp_micros;
  metadata.clock_domain = (uint32_t)clock_domain;
  metadata.uncertainty_micros = (uint32_t)uncertainty_micros;
  metadata.pose_device_local.position_meters.x = px;
  metadata.pose_device_local.position_meters.y = py;
  metadata.pose_device_local.position_meters.z = pz;
  metadata.pose_device_local.orientation.x = qx;
  metadata.pose_device_local.orientation.y = qy;
  metadata.pose_device_local.orientation.z = qz;
  metadata.pose_device_local.orientation.w = qw;
  metadata.intrinsics.width_px = (uint32_t)width_px;
  metadata.intrinsics.height_px = (uint32_t)height_px;
  metadata.intrinsics.fx = fx;
  metadata.intrinsics.fy = fy;
  metadata.intrinsics.cx = cx;
  metadata.intrinsics.cy = cy;

  /*
   * ARCore Frame does not expose exposure/ISO through this adapter yet.
   * Zero is accepted by the scanner-core metadata contract and means unavailable.
   */
  metadata.exposure_seconds = 0.0;
  metadata.iso = 0.0;
  metadata.optional_fields = 0;

  return g_symbols.camera(
      (ScannerNativeIngestSession *)(uintptr_t)handle, &metadata);
}

JNIEXPORT jint JNICALL
Java_com_xtreemze_scanner_sensors_ScannerCoreIngestBridge_nativeIngestImu(
    JNIEnv *env,
    jclass clazz,
    jlong handle,
    jlong timestamp_micros,
    jint clock_domain,
    jint uncertainty_micros,
    jdouble ax,
    jdouble ay,
    jdouble az,
    jdouble gx,
    jdouble gy,
    jdouble gz) {
  (void)env;
  (void)clazz;
  if (!ensure_symbols() || handle == 0) {
    return SCANNER_NATIVE_NULL_HANDLE;
  }
  if (timestamp_micros < 0 || uncertainty_micros < 0) {
    return SCANNER_NATIVE_INVALID_METADATA;
  }

  ScannerNativeImuSample sample = {0};
  sample.timestamp_micros = (uint64_t)timestamp_micros;
  sample.clock_domain = (uint32_t)clock_domain;
  sample.uncertainty_micros = (uint32_t)uncertainty_micros;
  sample.acceleration_mps2.x = ax;
  sample.acceleration_mps2.y = ay;
  sample.acceleration_mps2.z = az;
  sample.angular_velocity_rps.x = gx;
  sample.angular_velocity_rps.y = gy;
  sample.angular_velocity_rps.z = gz;
  sample.has_gravity = 0;

  return g_symbols.imu(
      (ScannerNativeIngestSession *)(uintptr_t)handle, &sample);
}

JNIEXPORT jint JNICALL
Java_com_xtreemze_scanner_sensors_ScannerCoreIngestBridge_nativeIngestDepth(
    JNIEnv *env,
    jclass clazz,
    jlong handle,
    jlong timestamp_micros,
    jint clock_domain,
    jint uncertainty_micros,
    jint width_px,
    jint height_px,
    jint depth_format,
    jobject depth_buffer,
    jint depth_row_stride_bytes,
    jobject confidence_buffer,
    jint confidence_row_stride_bytes,
    jboolean fresh_for_frame) {
  (void)clazz;
  if (!ensure_symbols() || handle == 0) {
    return SCANNER_NATIVE_NULL_HANDLE;
  }
  if (timestamp_micros < 0 || uncertainty_micros < 0 ||
      width_px <= 0 || height_px <= 0 ||
      depth_row_stride_bytes <= 0 || confidence_row_stride_bytes <= 0 ||
      depth_buffer == NULL || confidence_buffer == NULL) {
    return SCANNER_NATIVE_INVALID_METADATA;
  }

  uint8_t *depth = (uint8_t *)(*env)->GetDirectBufferAddress(env, depth_buffer);
  uint8_t *confidence =
      (uint8_t *)(*env)->GetDirectBufferAddress(env, confidence_buffer);
  jlong depth_capacity = (*env)->GetDirectBufferCapacity(env, depth_buffer);
  jlong confidence_capacity =
      (*env)->GetDirectBufferCapacity(env, confidence_buffer);

  if (depth == NULL || confidence == NULL ||
      depth_capacity <= 0 || confidence_capacity <= 0) {
    return SCANNER_NATIVE_INVALID_BUFFER;
  }

  ScannerNativeDepthDescriptor descriptor = {0};
  descriptor.timestamp_micros = (uint64_t)timestamp_micros;
  descriptor.clock_domain = (uint32_t)clock_domain;
  descriptor.uncertainty_micros = (uint32_t)uncertainty_micros;
  descriptor.width_px = (uint32_t)width_px;
  descriptor.height_px = (uint32_t)height_px;
  descriptor.depth_format = (uint32_t)depth_format;
  descriptor.depth_row_stride_bytes = (size_t)depth_row_stride_bytes;
  descriptor.confidence_row_stride_bytes = (size_t)confidence_row_stride_bytes;
  descriptor.min_depth_meters = 0.0f;
  descriptor.max_depth_meters = 0.0f;
  descriptor.fresh_for_frame = fresh_for_frame ? 1 : 0;

  return g_symbols.depth(
      (ScannerNativeIngestSession *)(uintptr_t)handle,
      &descriptor,
      depth,
      (size_t)depth_capacity,
      confidence,
      (size_t)confidence_capacity);
}

JNIEXPORT jint JNICALL
Java_com_xtreemze_scanner_sensors_ScannerCoreIngestBridge_nativeStats(
    JNIEnv *env, jclass clazz, jlong handle, jlongArray out_values) {
  (void)clazz;
  if (!ensure_symbols() || handle == 0) {
    return SCANNER_NATIVE_NULL_HANDLE;
  }
  if (out_values == NULL || (*env)->GetArrayLength(env, out_values) < 7) {
    return SCANNER_NATIVE_INVALID_BUFFER;
  }

  ScannerNativeIngestStats stats = {0};
  ScannerNativeStatus status =
      g_symbols.stats((ScannerNativeIngestSession *)(uintptr_t)handle, &stats);
  if (status != SCANNER_NATIVE_OK) {
    return status;
  }

  jlong values[7] = {
      (jlong)stats.accepted_camera_frames,
      (jlong)stats.accepted_imu_samples,
      (jlong)stats.accepted_depth_frames,
      (jlong)stats.dropped_observations,
      (jlong)stats.dropped_depth_frames,
      (jlong)stats.queued_observations,
      (jlong)stats.queued_depth_frames,
  };
  (*env)->SetLongArrayRegion(env, out_values, 0, 7, values);
  return SCANNER_NATIVE_OK;
}
