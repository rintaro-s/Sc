using System;
using System.Net;
using System.Net.Sockets;
using System.Text;
using UnityEngine;

namespace CRBridge.Integrations
{
    // CAS 共通型
    [Serializable] public struct CASVec3 {
        public float x, y, z;
        public CASVec3(float x, float y, float z){this.x=x;this.y=y;this.z=z;}
        public override string ToString() => $"({x:F4},{y:F4},{z:F4})";
    }
    [Serializable] public struct CASQuat {
        public float x, y, z, w;
        public CASQuat(float x, float y, float z, float w){this.x=x;this.y=y;this.z=z;this.w=w;}
    }
    [Serializable] public struct CASTransform {
        public string entity_id;
        public CASVec3 position;  public CASQuat rotation; public CASVec3 scale;
        public long timestamp_ms;
    }
    [Serializable] public struct CASBatch { public CASTransform[] transforms; }

    // Unity LH Y-up -> CAS RH Y-up
    // pos: (-x, y, z)   rot: (-qx, -qy, qz, qw)
    public static class CRBCoordinateConverter
    {
        public static CASVec3 ToCASPosition(Vector3 p)   => new CASVec3(-p.x, p.y, p.z);
        public static CASQuat ToCASRotation(Quaternion q) => new CASQuat(-q.x, -q.y, q.z, q.w);
        public static CASVec3 ToCASScale(Vector3 s)       => new CASVec3(s.x, s.y, s.z);
        public static Vector3    FromCASPosition(CASVec3 p) => new Vector3(-p.x, p.y, p.z);
        public static Quaternion FromCASRotation(CASQuat q) => new Quaternion(-q.x, -q.y, q.z, q.w);
        public static CASTransform FromTransform(Transform t, string id = null) => new CASTransform {
            entity_id    = id ?? t.gameObject.name,
            position     = ToCASPosition(t.position),
            rotation     = ToCASRotation(t.rotation),
            scale        = ToCASScale(t.lossyScale),
            timestamp_ms = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds(),
        };
        public static void ApplyToTransform(CASTransform ct, Transform t) {
            t.position = FromCASPosition(ct.position);
            t.rotation = FromCASRotation(ct.rotation);
        }
    }

    // MonoBehaviour: UDP 同期コンポーネント
    public class CRBSyncComponent : MonoBehaviour
    {
        [Header("CAS Bridge UDP")]
        public string host     = "127.0.0.1";
        public int    port     = 9101;
        [Range(0.016f, 2.0f)]
        public float  interval = 0.05f;
        [Header("Entity")]
        public string entityId = "";

        private UdpClient  _udp;
        private IPEndPoint _ep;
        private float      _t;

        void Start() {
            if (string.IsNullOrEmpty(entityId)) entityId = gameObject.name;
            try {
                _udp = new UdpClient();
                _ep  = new IPEndPoint(IPAddress.Parse(host), port);
                Debug.Log($"[CRB] UDP Sync -> {host}:{port} ({entityId})");
            } catch (Exception e) {
                Debug.LogError($"[CRB] UDP init: {e.Message}");
                enabled = false;
            }
        }
        void Update() {
            _t += Time.deltaTime;
            if (_t < interval) return;
            _t = 0f;
            if (_udp == null) return;
            var ct    = CRBCoordinateConverter.FromTransform(transform, entityId);
            var batch = new CASBatch { transforms = new[] { ct } };
            var bytes = Encoding.UTF8.GetBytes(JsonUtility.ToJson(batch));
            try { _udp.Send(bytes, bytes.Length, _ep); }
            catch (Exception e) { Debug.LogWarning($"[CRB] send: {e.Message}"); }
        }
        void OnDestroy() { _udp?.Close(); }
        void OnDrawGizmosSelected() {
            Gizmos.color = Color.cyan;
            Gizmos.DrawWireCube(transform.position, Vector3.one * 0.3f);
        }
    }

    // CAS sidecar JSON for GLB exports
    [Serializable]
    public class CASGLBMeta {
        public string cas_version      = "0.1";
        public string coordinate_space = "CAS";
        public bool   right_handed     = true;
        public string up_axis          = "+Y";
        public string forward_axis     = "+Z";
        public string unit             = "meter";
        public string quaternion_order = "[x,y,z,w]";
        public long   exported_at;
        public CASTransform[] transforms;
    }
}
