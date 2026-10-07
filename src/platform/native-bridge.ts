import type { SpatialCapability } from '../domain/models';

export interface PlatformCapabilities {
  readonly runtime: 'web' | 'tauri';
  readonly mobile: boolean;
  readonly capabilities: readonly SpatialCapability[];
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

export async function platformCapabilities(): Promise<PlatformCapabilities> {
  if (!('__TAURI_INTERNALS__' in window)) {
    return {
      runtime: 'web',
      mobile: matchMedia('(pointer: coarse)').matches,
      capabilities: browserCapabilities()
    };
  }

  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<PlatformCapabilities>('platform_capabilities');
}
