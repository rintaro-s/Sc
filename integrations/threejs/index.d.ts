/**
 * CR-Bridge CAS Adapter for Three.js
 * TypeScript type definitions
 */

// ─────────────────────────────────────────────────────────────
// CAS 型定義
// ─────────────────────────────────────────────────────────────

/** CAS 絶対座標変換 */
export interface CASTransform {
  entity_id:    string;
  position:     [number, number, number];
  rotation:     [number, number, number, number]; // [x,y,z,w]
  scale:        [number, number, number];
  timestamp_ms: number;
}

/** CAS バッチ */
export interface CASBatch {
  transforms: CASTransform[];
}

/** CAS 準拠チェック結果 */
export interface ValidationResult {
  valid:  boolean;
  issues: string[];
}

/** GLB エクスポート用メタデータ */
export interface CASGLBMeta {
  cas_version:      string;
  coordinate_space: 'CAS';
  right_handed:     true;
  up_axis:          '+Y';
  forward_axis:     '+Z';
  unit:             'meter';
  quaternion_order: '[x,y,z,w]';
  source:           string;
  exported_at:      number;
  transforms:       CASTransform[];
}

// ─────────────────────────────────────────────────────────────
// サーバープロトコル型
// ─────────────────────────────────────────────────────────────

export interface EntitySnapshot {
  entity_id:      string;
  display_name:   string;
  avatar_color:   string;
  position:       [number, number, number];
  rotation:       [number, number, number, number];
  velocity:       [number, number, number];
  last_update_ms: number;
}

export interface WorldStateMsg {
  type:       'world_state';
  world_id:   string;
  world_name: string;
  entities:   EntitySnapshot[];
}

// ─────────────────────────────────────────────────────────────
// 変換関数
// ─────────────────────────────────────────────────────────────

export function toCASPosition(v: { x: number; y: number; z: number }): [number, number, number];
export function toCASRotation(q: { x: number; y: number; z: number; w: number }): [number, number, number, number];
export function toCASScale   (s: { x: number; y: number; z: number }): [number, number, number];

export function fromCASPosition(p: [number, number, number], out?: { x: number; y: number; z: number }): { x: number; y: number; z: number };
export function fromCASRotation(r: [number, number, number, number], out?: { x: number; y: number; z: number; w: number }): { x: number; y: number; z: number; w: number };

export function toAbsoluteTransform(entityId: string, obj: { position: { x: number; y: number; z: number }; quaternion: { x: number; y: number; z: number; w: number }; scale: { x: number; y: number; z: number } }): CASTransform;

export function batchFromObjects<T extends object>(objects: T[], idOf: (obj: T) => string): CASBatch;

export function applyCASTransform(ct: CASTransform, obj: { position: { set(x: number, y: number, z: number): void }; quaternion: { set(x: number, y: number, z: number, w: number): void } }): void;

export function validateCASTransform(ct: CASTransform): ValidationResult;

export function sendBatchUDP(batch: CASBatch, host?: string, port?: number): void;

export function createGLBMetadata<T extends object>(objects: T[], idOf: (obj: T) => string, sourceApp?: string): CASGLBMeta;

// ─────────────────────────────────────────────────────────────
// WebSocket クライアント
// ─────────────────────────────────────────────────────────────

export interface CRBMetaverseClientOptions {
  url?:             string;
  reconnectDelay?:  number;
}

export class CRBMetaverseClient {
  constructor(options?: CRBMetaverseClientOptions);

  onConnected?:    () => void;
  onDisconnected?: () => void;
  onWorldState?:   (msg: WorldStateMsg) => void;
  onEntityJoined?: (entity: EntitySnapshot) => void;
  onEntityLeft?:   (entityId: string) => void;
  onEntityPose?:   (entityId: string, position: [number,number,number], rotation: [number,number,number,number], velocity: [number,number,number], timestampMs: number) => void;
  onChat?:         (fromId: string, fromName: string, text: string) => void;
  onPong?:         (clientTs: number, serverTs: number) => void;

  joinWorld(worldId: string, userId: string, displayName: string, avatarColor?: string): void;
  sendPose(position: [number,number,number], rotation: [number,number,number,number], velocity?: [number,number,number]): void;
  sendChat(text: string): void;
  ping(): void;
  send(msg: object): void;
}

// Legacy API
export { toCASPosition as threeToCASPosition };
export { toCASRotation as threeToCASQuaternion };
