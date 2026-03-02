/**
 * @file crbCoordinateAdapter.js
 * @description CR-Bridge CAS Coordinate Adapter for Three.js
 *
 * Three.js は Right-Handed, Y-up, +Z forward なので
 * CAS と完全一致。変換は恒等変換。
 *
 * CAS: Right-Handed, Y-up, +Z forward, meters, quaternion [x,y,z,w]
 */

// ─────────────────────────────────────────────────────────────
// 基本変換 (Three.js = CAS なので恒等)
// ─────────────────────────────────────────────────────────────

/** @param {THREE.Vector3} v @returns {[number,number,number]} */
export function toCASPosition(v) {
  return [v.x, v.y, v.z];
}

/** @param {THREE.Quaternion} q @returns {[number,number,number,number]} */
export function toCASRotation(q) {
  return [q.x, q.y, q.z, q.w];
}

/** @param {THREE.Vector3} s @returns {[number,number,number]} */
export function toCASScale(s) {
  return [s.x, s.y, s.z];
}

/** CAS position → Three.js Vector3
 * @param {[number,number,number]} p @param {THREE.Vector3} [out]
 * @returns {THREE.Vector3} */
export function fromCASPosition(p, out) {
  if (out) { out.set(p[0], p[1], p[2]); return out; }
  // THREE not imported here – caller handles
  return { x: p[0], y: p[1], z: p[2] };
}

/** CAS rotation → Three.js Quaternion
 * @param {[number,number,number,number]} r @param {THREE.Quaternion} [out]
 * @returns {THREE.Quaternion} */
export function fromCASRotation(r, out) {
  if (out) { out.set(r[0], r[1], r[2], r[3]); return out; }
  return { x: r[0], y: r[1], z: r[2], w: r[3] };
}

// ─────────────────────────────────────────────────────────────
// CASTransform オブジェクト
// ─────────────────────────────────────────────────────────────

/**
 * Three.js Object3D → CASTransform
 * @param {string} entityId
 * @param {THREE.Object3D} obj
 * @returns {Object} CASTransform
 */
export function toAbsoluteTransform(entityId, obj) {
  return {
    entity_id:    entityId,
    position:     toCASPosition(obj.position),
    rotation:     toCASRotation(obj.quaternion),
    scale:        toCASScale(obj.scale),
    timestamp_ms: Date.now(),
  };
}

/**
 * Object3D のリスト → CAS AbsoluteTransformBatch
 * @param {THREE.Object3D[]} objects
 * @param {function(THREE.Object3D): string} idOf entity_id 解決関数
 * @returns {Object} AbsoluteTransformBatch
 */
export function batchFromObjects(objects, idOf) {
  return {
    transforms: objects.map(o => toAbsoluteTransform(idOf(o), o)),
  };
}

/**
 * CASTransform を Object3D に適用 (位置・回転)
 * @param {Object} ct CASTransform
 * @param {THREE.Object3D} obj
 */
export function applyCASTransform(ct, obj) {
  const [px, py, pz]    = ct.position;
  const [qx, qy, qz, qw] = ct.rotation;
  obj.position.set(px, py, pz);
  obj.quaternion.set(qx, qy, qz, qw);
}

// ─────────────────────────────────────────────────────────────
// CAS 準拠検証
// ─────────────────────────────────────────────────────────────

/** CASTransform の基本検証 */
export function validateCASTransform(ct) {
  const issues = [];
  if (!Array.isArray(ct.position) || ct.position.length !== 3)
    issues.push('position must be [x,y,z]');
  if (!Array.isArray(ct.rotation) || ct.rotation.length !== 4)
    issues.push('rotation must be [x,y,z,w]');
  if (ct.rotation) {
    const [x,y,z,w] = ct.rotation;
    const len = Math.sqrt(x*x + y*y + z*z + w*w);
    if (Math.abs(len - 1) > 0.01) issues.push(`rotation not unit: |q|=${len.toFixed(4)}`);
  }
  return { valid: issues.length === 0, issues };
}

// ─────────────────────────────────────────────────────────────
// UDP ブロードキャスト (Node.js 環境)
// ─────────────────────────────────────────────────────────────

/** Node.js 環境でのみ動作する UDP 送信 */
export function sendBatchUDP(batch, host = '127.0.0.1', port = 9101) {
  if (typeof require === 'undefined') {
    console.warn('[CRB] sendBatchUDP: Node.js 環境でのみ動作します');
    return;
  }
  const dgram  = require('dgram');
  const msg    = Buffer.from(JSON.stringify(batch), 'utf-8');
  const client = dgram.createSocket('udp4');
  client.send(msg, port, host, () => client.close());
}

// ─────────────────────────────────────────────────────────────
// WebSocket クライアント (ブラウザ / Node.js 共通)
// ─────────────────────────────────────────────────────────────

/**
 * CR-Bridge Metaverse Server への WebSocket クライアント
 * @example
 * const client = new CRBMetaverseClient({ url: 'ws://localhost:8080/ws' });
 * client.joinWorld('default', 'user1', 'Alice', '#ff4488');
 * client.onEntityPose = (id, pos, rot, vel) => { ... };
 */
export class CRBMetaverseClient {
  constructor({ url, reconnectDelay = 3000 } = {}) {
    this.url            = url || `ws://${location.host}/ws`;
    this.reconnectDelay = reconnectDelay;
    this.ws             = null;
    this._joined        = null;

    // コールバック
    this.onConnected    = null;
    this.onDisconnected = null;
    this.onWorldState   = null;
    this.onEntityJoined = null;
    this.onEntityLeft   = null;
    this.onEntityPose   = null;
    this.onChat         = null;
    this.onPong         = null;

    this._connect();
  }

  _connect() {
    this.ws = new WebSocket(this.url);
    this.ws.onopen    = () => {
      if (this._joined) this.joinWorld(...this._joined);
      this.onConnected?.();
    };
    this.ws.onmessage = (ev) => this._handle(JSON.parse(ev.data));
    this.ws.onclose   = () => {
      this.onDisconnected?.();
      setTimeout(() => this._connect(), this.reconnectDelay);
    };
  }

  _handle(msg) {
    switch (msg.type) {
      case 'world_state':   this.onWorldState?.(msg);                                    break;
      case 'entity_joined': this.onEntityJoined?.(msg.entity);                          break;
      case 'entity_left':   this.onEntityLeft?.(msg.entity_id);                         break;
      case 'entity_pose':   this.onEntityPose?.(msg.entity_id, msg.position, msg.rotation, msg.velocity, msg.timestamp_ms); break;
      case 'chat_message':  this.onChat?.(msg.from_id, msg.from_name, msg.text);        break;
      case 'pong':          this.onPong?.(msg.client_timestamp_ms, msg.server_timestamp_ms); break;
    }
  }

  send(msg) {
    if (this.ws?.readyState === WebSocket.OPEN)
      this.ws.send(JSON.stringify(msg));
  }

  joinWorld(worldId, userId, displayName, avatarColor) {
    this._joined = [worldId, userId, displayName, avatarColor];
    this.send({ type: 'join_world', world_id: worldId, user_id: userId,
                display_name: displayName, avatar_color: avatarColor });
  }

  sendPose(position, rotation, velocity = [0,0,0]) {
    this.send({ type: 'update_pose', position, rotation, velocity,
                timestamp_ms: Date.now() });
  }

  sendChat(text) { this.send({ type: 'chat', text }); }

  ping() {
    this.send({ type: 'ping', client_timestamp_ms: Date.now() });
  }
}

// ─────────────────────────────────────────────────────────────
// GLB メタデータ
// ─────────────────────────────────────────────────────────────

/** GLB エクスポート時の CAS sidecar JSON を生成 */
export function createGLBMetadata(objects, idOf, sourceApp = 'Three.js') {
  return {
    cas_version:       '0.1',
    coordinate_space:  'CAS',
    right_handed:      true,
    up_axis:           '+Y',
    forward_axis:      '+Z',
    unit:              'meter',
    quaternion_order:  '[x,y,z,w]',
    source:            sourceApp,
    exported_at:       Date.now(),
    transforms:        objects.map(o => toAbsoluteTransform(idOf(o), o)),
  };
}

// Legacy API (後方互換)
export const threeToCASPosition = toCASPosition;
export const threeToCASQuaternion = toCASRotation;

