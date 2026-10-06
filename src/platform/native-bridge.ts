import type { SpatialCapability } from '../domain/models';

export interface PlatformCapabilities {
  readonly runtime: 'web' | 'tauri';
  readonly mobile: boolean;
  readonly capabilities: readonly SpatialCapability[];
}

export async function platformCapabilities(): Promise<PlatformCapabilities> {
  if (!('__TAURI_INTERNALS__' in window)) {
    const capabilities: SpatialCapability[] = ['camera', 'imu'];
    return {
      runtime: 'web',
      mobile: matchMedia('(pointer: coarse)').matches,
      capabilities
    };
  }

  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<PlatformCapabilities>('platform_capabilities');
}
