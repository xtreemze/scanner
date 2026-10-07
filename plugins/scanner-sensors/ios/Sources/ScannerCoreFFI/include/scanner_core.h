#ifndef SCANNER_CORE_H
#define SCANNER_CORE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct ScannerNativeIngestSession ScannerNativeIngestSession;

typedef enum ScannerNativeStatus {
  SCANNER_NATIVE_OK = 0,
  SCANNER_NATIVE_NULL_HANDLE = 1,
  SCANNER_NATIVE_NULL_POINTER = 2,
  SCANNER_NATIVE_INVALID_UTF8 = 3,
  SCANNER_NATIVE_INVALID_CLOCK_DOMAIN = 4,
  SCANNER_NATIVE_INVALID_METADATA = 5,
  SCANNER_NATIVE_INVALID_BUFFER = 6,
  SCANNER_NATIVE_DUPLICATE_OBSERVATION = 7,
  SCANNER_NATIVE_STALE_EPOCH = 8
} ScannerNativeStatus;

typedef enum ScannerNativeDepthFormat {
  SCANNER_NATIVE_DEPTH_U16_MILLIMETERS = 1,
  SCANNER_NATIVE_DEPTH_F32_METERS = 2
} ScannerNativeDepthFormat;

typedef struct ScannerNativeVec3 {
  double x;
  double y;
  double z;
} ScannerNativeVec3;

typedef struct ScannerNativeQuaternion {
  double x;
  double y;
  double z;
  double w;
} ScannerNativeQuaternion;

typedef struct ScannerNativePose {
  ScannerNativeVec3 position_meters;
  ScannerNativeQuaternion orientation;
} ScannerNativePose;

typedef struct ScannerNativeCameraIntrinsics {
  uint32_t width_px;
  uint32_t height_px;
  double fx;
  double fy;
  double cx;
  double cy;
} ScannerNativeCameraIntrinsics;

typedef struct ScannerNativeCameraFrameMetadata {
  uint64_t timestamp_micros;
  uint32_t clock_domain;
  uint32_t uncertainty_micros;
  ScannerNativePose pose_device_local;
  ScannerNativeCameraIntrinsics intrinsics;
  double exposure_seconds;
  double iso;
  double aperture_f_number;
  double white_balance_kelvin;
  uint32_t optional_fields;
} ScannerNativeCameraFrameMetadata;

typedef struct ScannerNativeImuSample {
  uint64_t timestamp_micros;
  uint32_t clock_domain;
  uint32_t uncertainty_micros;
  ScannerNativeVec3 acceleration_mps2;
  ScannerNativeVec3 angular_velocity_rps;
  ScannerNativeVec3 gravity_mps2;
  uint8_t has_gravity;
} ScannerNativeImuSample;

typedef struct ScannerNativeDepthDescriptor {
  uint64_t timestamp_micros;
  uint32_t clock_domain;
  uint32_t uncertainty_micros;
  uint32_t width_px;
  uint32_t height_px;
  uint32_t depth_format;
  size_t depth_row_stride_bytes;
  size_t confidence_row_stride_bytes;
  float min_depth_meters;
  float max_depth_meters;
  uint8_t fresh_for_frame;
} ScannerNativeDepthDescriptor;

typedef struct ScannerNativeIngestStats {
  uint64_t accepted_camera_frames;
  uint64_t accepted_imu_samples;
  uint64_t accepted_depth_frames;
  uint64_t dropped_observations;
  uint64_t dropped_depth_frames;
  size_t queued_observations;
  size_t queued_depth_frames;
} ScannerNativeIngestStats;

/* Clock domain values match scanner_core::ClockDomain:
 * 0 session-monotonic, 1 device-monotonic, 2 camera-sensor, 3 platform.
 */
enum {
  SCANNER_CLOCK_SESSION_MONOTONIC = 0,
  SCANNER_CLOCK_DEVICE_MONOTONIC = 1,
  SCANNER_CLOCK_CAMERA_SENSOR = 2,
  SCANNER_CLOCK_PLATFORM = 3
};

enum {
  SCANNER_CAMERA_OPTIONAL_APERTURE = 1u << 0,
  SCANNER_CAMERA_OPTIONAL_WHITE_BALANCE = 1u << 1
};

ScannerNativeIngestSession *scanner_native_session_create(
    const uint8_t *device_id_ptr,
    size_t device_id_len,
    uint64_t epoch);

void scanner_native_session_destroy(ScannerNativeIngestSession *handle);

ScannerNativeStatus scanner_native_ingest_camera_metadata(
    ScannerNativeIngestSession *handle,
    const ScannerNativeCameraFrameMetadata *metadata);

ScannerNativeStatus scanner_native_ingest_imu(
    ScannerNativeIngestSession *handle,
    const ScannerNativeImuSample *sample);

ScannerNativeStatus scanner_native_ingest_depth(
    ScannerNativeIngestSession *handle,
    const ScannerNativeDepthDescriptor *descriptor,
    const uint8_t *depth_ptr,
    size_t depth_len,
    const uint8_t *confidence_ptr,
    size_t confidence_len);

ScannerNativeStatus scanner_native_ingest_stats(
    ScannerNativeIngestSession *handle,
    ScannerNativeIngestStats *out_stats);

#ifdef __cplusplus
}
#endif

#endif
