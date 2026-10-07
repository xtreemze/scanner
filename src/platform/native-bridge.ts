import type { SpatialCapability } from '../domain/models';

export interface PlatformCapabilities {
  readonly runtime: 'web' | 'tauri';
  readonly mobile: boolean;
  readonly capabilities: readonly SpatialCapability[];
}

export interface SensorCapabilities {
  readonly camera: boolean;
  readonly imu: boolean;
  readonly depth: boolean;
  readonly lidar: boolean;
  readonly sceneMesh: boolean;
  readonly flashHardware: boolean;
  readonly flashSessionControl: boolean;
  readonly hdr: boolean;
  readonly platformTracking: boolean;
}

export type PermissionState = 'prompt' | 'granted' | 'denied';

export interface SensorPermissionState {
  readonly camera: PermissionState;
}

export interface StartSensorSessionOptions {
  readonly resetTracking: boolean;
  readonly preferRawDepth: boolean;
}

function browserCapabilities(): SpatialCapability[] {
  const capabilities: SpatialCapability[] = [];

  if (typeof navigator.mediaDevices?.getUserMedia === 'function') {
    capabilities.push('camera');
  }

  if ('DeviceMotionEvent' in window || 'Accelerometer' in window) {
    capabilities.push('imu');
  }

  return capabilities;
}

function isTauri(): boolean {
  return '__TAURI_INTERNALS__' in window;
}

async function invokeNative<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    throw new Error(`${command} is only available in the native Scanner app`);
  }

  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(command, args);
}

export async function platformCapabilities(): Promise<PlatformCapabilities> {
  if (!isTauri()) {
    return {
      runtime: 'web',
      mobile: matchMedia('(pointer: coarse)').matches,
      capabilities: browserCapabilities()
    };
  }

  return invokeNative<PlatformCapabilities>('platform_capabilities');
}

export function sensorCapabilities(): Promise<SensorCapabilities> {
  return invokeNative<SensorCapabilities>('sensor_capabilities');
}

export function sensorPermissions(): Promise<SensorPermissionState> {
  return invokeNative<SensorPermissionState>('sensor_permissions');
}

export function requestSensorPermissions(): Promise<SensorPermissionState> {
  return invokeNative<SensorPermissionState>('request_sensor_permissions');
}

export function startSensorSession(
  options: StartSensorSessionOptions = {
    resetTracking: false,
    preferRawDepth: true
  }
): Promise<void> {
  return invokeNative<void>('start_sensor_session', { options });
}

export function stopSensorSession(): Promise<void> {
  return invokeNative<void>('stop_sensor_session');
}

export function setSensorTorch(level: number | null): Promise<void> {
  return invokeNative<void>('set_sensor_torch', {
    options: { level }
  });
}
