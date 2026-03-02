/*
 * CR-Bridge Unity Editor Window
 * ================================
 * CRBSyncComponent の管理UI
 * シーン内の全同期オブジェクトの状態を一覧表示
 */

using UnityEngine;
using UnityEditor;
using System.Collections.Generic;
using System.Linq;

namespace CrBridge.Editor
{
    public class CRBEditorWindow : EditorWindow
    {
        private Vector2 scrollPosition;
        private bool autoRefresh = true;
        private double lastRefreshTime = 0;
        private const double REFRESH_INTERVAL = 0.5; // 0.5秒ごとに更新

        [MenuItem("Window/CR-Bridge/Sync Manager")]
        public static void ShowWindow()
        {
            var window = GetWindow<CRBEditorWindow>("CR-Bridge Sync");
            window.minSize = new Vector2(400, 300);
        }

        private void OnEnable()
        {
            EditorApplication.update += OnEditorUpdate;
        }

        private void OnDisable()
        {
            EditorApplication.update -= OnEditorUpdate;
        }

        private void OnEditorUpdate()
        {
            if (autoRefresh && EditorApplication.timeSinceStartup - lastRefreshTime > REFRESH_INTERVAL)
            {
                Repaint();
                lastRefreshTime = EditorApplication.timeSinceStartup;
            }
        }

        private void OnGUI()
        {
            DrawHeader();
            DrawSyncList();
            DrawFooter();
        }

        private void DrawHeader()
        {
            EditorGUILayout.Space(5);
            
            GUIStyle titleStyle = new GUIStyle(EditorStyles.boldLabel)
            {
                fontSize = 16,
                alignment = TextAnchor.MiddleCenter
            };
            EditorGUILayout.LabelField("CR-Bridge Sync Manager", titleStyle);
            
            EditorGUILayout.Space(5);
            EditorGUILayout.BeginHorizontal(EditorStyles.helpBox);
            EditorGUILayout.LabelField("🌐 CAS Coordinate Synchronization", EditorStyles.miniBoldLabel);
            EditorGUILayout.EndHorizontal();
            
            EditorGUILayout.Space(10);
        }

        private void DrawSyncList()
        {
            var syncComponents = FindObjectsOfType<CRBSyncComponent>();
            
            EditorGUILayout.BeginVertical(EditorStyles.helpBox);
            EditorGUILayout.LabelField($"Active Sync Components: {syncComponents.Length}", EditorStyles.boldLabel);
            EditorGUILayout.EndVertical();

            if (syncComponents.Length == 0)
            {
                EditorGUILayout.Space(10);
                EditorGUILayout.HelpBox(
                    "シーンに CRBSyncComponent が見つかりません。\n" +
                    "GameObjectを選択して Add Component > CRBSyncComponent を追加してください。",
                    MessageType.Info
                );
                return;
            }

            EditorGUILayout.Space(5);
            scrollPosition = EditorGUILayout.BeginScrollView(scrollPosition);

            foreach (var comp in syncComponents.OrderBy(c => c.gameObject.name))
            {
                DrawSyncComponent(comp);
                EditorGUILayout.Space(3);
            }

            EditorGUILayout.EndScrollView();
        }

        private void DrawSyncComponent(CRBSyncComponent comp)
        {
            EditorGUILayout.BeginVertical(EditorStyles.helpBox);
            
            // ヘッダー行
            EditorGUILayout.BeginHorizontal();
            
            // アクティブ状態アイコン
            string statusIcon = comp.enabled ? "✅" : "⚪";
            EditorGUILayout.LabelField(statusIcon, GUILayout.Width(20));
            
            // オブジェクト名（クリックで選択）
            if (GUILayout.Button(comp.gameObject.name, EditorStyles.linkLabel))
            {
                Selection.activeGameObject = comp.gameObject;
                EditorGUIUtility.PingObject(comp.gameObject);
            }
            
            GUILayout.FlexibleSpace();
            
            // Enable/Disable トグル
            bool newEnabled = EditorGUILayout.Toggle(comp.enabled, GUILayout.Width(20));
            if (newEnabled != comp.enabled)
            {
                Undo.RecordObject(comp, "Toggle CRBSync");
                comp.enabled = newEnabled;
                EditorUtility.SetDirty(comp);
            }
            
            EditorGUILayout.EndHorizontal();

            // 詳細情報
            EditorGUI.indentLevel++;
            
            EditorGUILayout.BeginHorizontal();
            EditorGUILayout.LabelField("Target", GUILayout.Width(80));
            EditorGUILayout.LabelField($"{comp.targetIP}:{comp.targetPort}", EditorStyles.miniLabel);
            EditorGUILayout.EndHorizontal();

            EditorGUILayout.BeginHorizontal();
            EditorGUILayout.LabelField("Update Rate", GUILayout.Width(80));
            EditorGUILayout.LabelField($"{comp.updateRate} Hz", EditorStyles.miniLabel);
            EditorGUILayout.EndHorizontal();

            // CAS座標プレビュー
            if (Application.isPlaying)
            {
                var casPos = CRBCoordinateConverter.ToCASPosition(comp.transform.position);
                var casRot = CRBCoordinateConverter.ToCASRotation(comp.transform.rotation);
                
                EditorGUILayout.BeginHorizontal();
                EditorGUILayout.LabelField("CAS Pos", GUILayout.Width(80));
                EditorGUILayout.LabelField(
                    $"({casPos.x:F2}, {casPos.y:F2}, {casPos.z:F2})",
                    EditorStyles.miniLabel
                );
                EditorGUILayout.EndHorizontal();

                EditorGUILayout.BeginHorizontal();
                EditorGUILayout.LabelField("CAS Rot", GUILayout.Width(80));
                EditorGUILayout.LabelField(
                    $"({casRot.x:F2}, {casRot.y:F2}, {casRot.z:F2}, {casRot.w:F2})",
                    EditorStyles.miniLabel
                );
                EditorGUILayout.EndHorizontal();
            }
            else
            {
                EditorGUILayout.HelpBox("Play Mode で座標が表示されます", MessageType.None);
            }

            EditorGUI.indentLevel--;
            
            EditorGUILayout.EndVertical();
        }

        private void DrawFooter()
        {
            EditorGUILayout.Space(10);
            EditorGUILayout.BeginHorizontal(EditorStyles.helpBox);
            
            EditorGUILayout.LabelField("Auto Refresh:", GUILayout.Width(90));
            autoRefresh = EditorGUILayout.Toggle(autoRefresh, GUILayout.Width(20));
            
            GUILayout.FlexibleSpace();
            
            if (GUILayout.Button("Refresh Now", GUILayout.Width(100)))
            {
                Repaint();
            }

            if (GUILayout.Button("Select All", GUILayout.Width(80)))
            {
                var syncComponents = FindObjectsOfType<CRBSyncComponent>();
                Selection.objects = syncComponents.Select(c => c.gameObject).ToArray();
            }

            EditorGUILayout.EndHorizontal();

            EditorGUILayout.Space(5);
            
            // フッター情報
            EditorGUILayout.BeginVertical(EditorStyles.helpBox);
            GUIStyle miniStyle = new GUIStyle(EditorStyles.miniLabel)
            {
                alignment = TextAnchor.MiddleCenter
            };
            EditorGUILayout.LabelField("CR-Bridge v0.4.0 | CAS Coordinate Space", miniStyle);
            EditorGUILayout.LabelField("Right-Handed | +Y Up | +Z Forward | [x,y,z,w] Quaternion", miniStyle);
            EditorGUILayout.EndVertical();
        }
    }

    /// <summary>
    /// Custom Inspector for CRBSyncComponent
    /// </summary>
    [CustomEditor(typeof(CRBSyncComponent))]
    public class CRBSyncComponentEditor : UnityEditor.Editor
    {
        public override void OnInspectorGUI()
        {
            DrawDefaultInspector();

            EditorGUILayout.Space(10);
            EditorGUILayout.HelpBox(
                "CAS (CR-Absolute-Space) 座標統一システム\n" +
                "Unity Left-Handed → CAS Right-Handed 変換\n\n" +
                "変換式:\n" +
                "  Position: (-x, y, z)\n" +
                "  Rotation: (-qx, -qy, qz, qw)",
                MessageType.Info
            );

            if (GUILayout.Button("Open CR-Bridge Manager"))
            {
                CRBEditorWindow.ShowWindow();
            }
        }
    }
}
