export type Confidence = number;

export type LockState =
  | 'candidate'
  | 'stable'
  | 'suggested-lock'
  | 'user-confirmed'
  | 'locked'
  | 'challenged';

export interface Vec3 {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

export interface Pose {
  readonly position: Vec3;
  readonly orientation: readonly [number, number, number, number];
  readonly timestampUs: number;
}

export interface StructuralSurface {
  readonly id: string;
  readonly kind: 'floor' | 'wall' | 'ceiling' | 'surface' | 'unknown';
  readonly normal: Vec3;
  readonly offsetMeters: number;
  readonly confidence: Confidence;
  readonly lockState: LockState;
}

export interface SpatialPeer {
  readonly id: string;
  readonly role: 'scanner' | 'anchor' | 'observer' | 'illuminator';
  readonly capabilities: readonly SpatialCapability[];
  readonly poseConfidence: Confidence;
}

export type SpatialCapability =
  | 'camera'
  | 'imu'
  | 'depth'
  | 'lidar'
  | 'uwb'
  | 'bluetooth-ranging'
  | 'flash'
  | 'hdr';

export interface ConfidenceField {
  readonly geometry: Confidence;
  readonly texture: Confidence;
  readonly pose: Confidence;
  readonly material: Confidence;
  readonly coverage: Confidence;
}

export type MeasurementAction =
  | { readonly kind: 'move-scanner'; readonly direction: Vec3; readonly meters: number }
  | { readonly kind: 'change-distance'; readonly meters: number }
  | { readonly kind: 'observe-surface'; readonly surfaceId: string }
  | { readonly kind: 'move-anchor'; readonly peerId: string; readonly direction: Vec3; readonly meters: number }
  | { readonly kind: 'hold-still'; readonly milliseconds: number }
  | { readonly kind: 'confirm-surface'; readonly surfaceId: string }
  | { readonly kind: 'lock-surface'; readonly surfaceId: string }
  | { readonly kind: 'rescan-region'; readonly regionId: string }
  | { readonly kind: 'illuminate-region'; readonly regionId: string; readonly peerId?: string };

export interface ScanSession {
  readonly id: string;
  readonly epoch: number;
  readonly peers: readonly SpatialPeer[];
  readonly surfaces: readonly StructuralSurface[];
  readonly confidence: ConfidenceField;
}
