#!/usr/bin/env python3
"""
CR-Bridge E2E Integration Test
==============================
metaverse-serverの実際の動作をテスト
- WebSocket接続確認
- 複数クライアント同時接続
- ポーズ更新・ブロードキャスト確認
- チャットメッセージ確認
"""

import asyncio
import json
import sys
import time
from datetime import datetime

try:
    import websockets
except ImportError:
    print("❌ websockets not found. Install: pip install websockets")
    sys.exit(1)

# Configuration
SERVER_URL = "ws://localhost:8082/ws"
TEST_TIMEOUT = 10  # seconds

class MetaverseClient:
    """Metaverse WebSocket Client"""

    def __init__(self, name: str, color: str):
        self.name = name
        self.color = color
        self.session_id = None
        self.entity_id = None
        self.websocket = None
        self.messages = []
        self.received_events = []

    async def connect(self):
        """Connect to metaverse server"""
        try:
            self.websocket = await asyncio.wait_for(
                websockets.connect(SERVER_URL),
                timeout=5
            )
            print(f"✅ [{self.name}] Connected to {SERVER_URL}")
            
            # Start listening
            asyncio.create_task(self._listen())
            
            return True
        except asyncio.TimeoutError:
            print(f"❌ [{self.name}] Connection timeout")
            return False
        except Exception as e:
            print(f"❌ [{self.name}] Connection failed: {e}")
            return False

    async def _listen(self):
        """Listen for server messages"""
        try:
            async for message in self.websocket:
                data = json.loads(message)
                self.received_events.append(data)
                
                # Handle specific message types
                if data.get("type") == "welcome":
                    self.session_id = data.get("session_id")
                    print(f"  📌 [{self.name}] Session: {self.session_id}")
                
                elif data.get("type") == "world_state":
                    entities = data.get("entities", [])
                    print(f"  🌍 [{self.name}] World state: {len(entities)} entities")
                
                elif data.get("type") == "entity_pose":
                    entity_id = data.get("entity_id")
                    pos = data.get("position", [0, 0, 0])
                    print(f"  📍 [{self.name}] Pose from {entity_id}: {pos}")
                
                elif data.get("type") == "chat_message":
                    sender = data.get("sender", "Unknown")
                    text = data.get("text", "")
                    print(f"  💬 [{self.name}] Chat from {sender}: {text}")
                
                elif data.get("type") == "pong":
                    latency = data.get("latency_ms", 0)
                    print(f"  🏓 [{self.name}] Ping latency: {latency}ms")
        
        except asyncio.CancelledError:
            pass
        except Exception as e:
            print(f"  ⚠️  [{self.name}] Listen error: {e}")

    async def join_world(self, world_id: str = "default"):
        """Join a world"""
        msg = {
            "type": "join_world",
            "world_id": world_id,
            "user_id": self.name,
            "display_name": self.name,
            "avatar_color": self.color,
        }
        await self.websocket.send(json.dumps(msg))
        print(f"  → [{self.name}] Join world: {world_id}")
        
        # Wait for world_state
        await asyncio.sleep(1)

    async def update_pose(self, pos: list, rot: list, vel: list = None):
        """Update entity pose"""
        if vel is None:
            vel = [0, 0, 0]
        
        msg = {
            "type": "update_pose",
            "position": pos,
            "rotation": rot,
            "velocity": vel,
            "timestamp_ms": int(time.time() * 1000),
        }
        await self.websocket.send(json.dumps(msg))

    async def send_chat(self, text: str):
        """Send chat message"""
        msg = {
            "type": "chat",
            "text": text,
            "timestamp_ms": int(time.time() * 1000),
        }
        await self.websocket.send(json.dumps(msg))
        print(f"  → [{self.name}] Chat: {text}")

    async def ping(self):
        """Send ping"""
        msg = {
            "type": "ping",
            "client_timestamp_ms": int(time.time() * 1000),
        }
        await self.websocket.send(json.dumps(msg))

    async def disconnect(self):
        """Disconnect"""
        if self.websocket:
            await self.websocket.close()
        print(f"🔌 [{self.name}] Disconnected")

    def get_received_count(self, msg_type: str) -> int:
        """Count received messages of type"""
        return len([e for e in self.received_events if e.get("type") == msg_type])


async def test_basic_connection():
    """Test 1: Basic connection"""
    print("\n" + "=" * 60)
    print("Test 1: Basic Connection")
    print("=" * 60)
    
    client = MetaverseClient("TestUser", "#667eea")
    if await client.connect():
        await asyncio.sleep(1)
        assert client.session_id, "No session_id received"
        print("✅ Test 1 passed")
        await client.disconnect()
        return True
    return False


async def test_world_join():
    """Test 2: World join and state"""
    print("\n" + "=" * 60)
    print("Test 2: World Join")
    print("=" * 60)
    
    client = MetaverseClient("User1", "#667eea")
    if not await client.connect():
        return False
    
    # Wait for welcome
    await asyncio.sleep(0.2)
    
    await client.join_world("default")
    
    # Wait longer for world_state (network + async processing time)
    await asyncio.sleep(1.5)
    
    world_count = client.get_received_count("world_state")
    assert world_count > 0, "No world_state received"
    print(f"✅ Test 2 passed ({world_count} world_state messages)")
    
    await client.disconnect()
    return True


async def test_multi_client_sync():
    """Test 3: Multiple clients and pose sync"""
    print("\n" + "=" * 60)
    print("Test 3: Multi-Client Pose Synchronization")
    print("=" * 60)
    
    # Create 2 clients
    client1 = MetaverseClient("Alice", "#667eea")
    client2 = MetaverseClient("Bob", "#764ba2")
    
    # Connect both
    if not (await client1.connect() and await client2.connect()):
        return False
    
    await asyncio.sleep(0.3)
    
    # Both join world
    await client1.join_world("default")
    await asyncio.sleep(1)
    
    await client2.join_world("default")
    await asyncio.sleep(1)
    
    # Client 1 sends pose
    await client1.update_pose([5, 0, 10], [0, 0, 0, 1])
    await asyncio.sleep(1.5)
    
    # Check if client 2 received pose update
    pose_count_c2 = client2.get_received_count("entity_pose")
    
    print(f"  Client 1 sent pose")
    print(f"  Client 2 received {pose_count_c2} entity_pose messages")
    
    if pose_count_c2 > 0:
        print("✅ Test 3 passed (Pose sync working)")
        success = True
    else:
        print("⚠️  Test 3: No pose received (may be interest management)")
        success = False
    
    await asyncio.gather(client1.disconnect(), client2.disconnect())
    return success


async def test_chat():
    """Test 4: Chat messaging"""
    print("\n" + "=" * 60)
    print("Test 4: Chat Messaging")
    print("=" * 60)
    
    client1 = MetaverseClient("Alice", "#667eea")
    client2 = MetaverseClient("Bob", "#764ba2")
    
    if not (await client1.connect() and await client2.connect()):
        return False
    
    await asyncio.sleep(0.3)
    
    await client1.join_world("default")
    await asyncio.sleep(1)
    
    await client2.join_world("default")
    await asyncio.sleep(1)
    
    # Send chat from client1
    await client1.send_chat("Hello Bob!")
    await asyncio.sleep(1.5)
    
    # Check if client2 received
    chat_count_c2 = client2.get_received_count("chat_message")
    
    if chat_count_c2 > 0:
        print("✅ Test 4 passed (Chat working)")
        success = True
    else:
        print("⚠️  Test 4: No chat received")
        success = False
    
    await asyncio.gather(client1.disconnect(), client2.disconnect())
    return success


async def test_ping():
    """Test 5: Ping/Latency"""
    print("\n" + "=" * 60)
    print("Test 5: Ping/Latency")
    print("=" * 60)
    
    client = MetaverseClient("ClientWithPing", "#667eea")
    if not await client.connect():
        return False
    
    await asyncio.sleep(0.3)
    
    await client.join_world("default")
    await asyncio.sleep(1)
    
    # Send ping
    await client.ping()
    await asyncio.sleep(1.5)
    
    pong_count = client.get_received_count("pong")
    
    if pong_count > 0:
        print("✅ Test 5 passed (Ping working)")
        success = True
    else:
        print("⚠️  Test 5: No pong received")
        success = False
    
    await client.disconnect()
    return success


async def main():
    """Run all tests"""
    print("\n" + "🌐" * 30)
    print("CR-Bridge Metaverse E2E Integration Test")
    print("🌐" * 30)
    
    # Check if server is running
    print("\n↪️  Checking if metaverse-server is running...")
    try:
        async with asyncio.timeout(2):
            async with websockets.connect(SERVER_URL) as ws:
                await ws.close()
            print("✅ Server is running\n")
    except Exception as e:
        print(f"❌ Server is not running at {SERVER_URL}")
        print(f"   Error: {e}")
        print(f"\n   Start server with: ./cr-launch start")
        return 1
    
    results = []
    tests = [
        ("Basic Connection", test_basic_connection),
        ("World Join", test_world_join),
        ("Multi-Client Sync", test_multi_client_sync),
        ("Chat Messaging", test_chat),
        ("Ping/Latency", test_ping),
    ]
    
    for name, test_func in tests:
        try:
            result = await asyncio.wait_for(test_func(), timeout=TEST_TIMEOUT)
            results.append((name, result))
        except asyncio.TimeoutError:
            print(f"\n❌ {name}: TIMEOUT")
            results.append((name, False))
        except Exception as e:
            print(f"\n❌ {name}: ERROR - {e}")
            results.append((name, False))
    
    # Summary
    print("\n" + "=" * 60)
    print("TEST SUMMARY")
    print("=" * 60)
    
    passed = sum(1 for _, result in results if result)
    total = len(results)
    
    for name, result in results:
        status = "✅ PASS" if result else "❌ FAIL"
        print(f"{status}: {name}")
    
    print(f"\nTotal: {passed}/{total} tests passed")
    print("=" * 60)
    
    return 0 if passed == total else 1


if __name__ == "__main__":
    try:
        exit_code = asyncio.run(main())
        sys.exit(exit_code)
    except KeyboardInterrupt:
        print("\n\n⚠️  Tests interrupted by user")
        sys.exit(130)
