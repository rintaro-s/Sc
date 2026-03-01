# CR-Bridge eBPF プローブ

## 概要

XDP (eXpress Data Path) フックを使って、NICドライバ直後にVRChat OSCパケットをカーネル内でフィルタリングします。

## 設計

```
NIC → XDP フック (カーネル内処理)
  ↓ VRChat OSCパケットのみ
perf_event ring buffer
  ↓ epoll_wait (1回のコピーのみ)
ATPエンジン (ユーザーランド)
```

## コピー回数の比較

| 方法 | コピー回数 |
|---|---|
| 通常のソケット受信 | NIC→カーネルバッファ→ユーザーバッファ (2回) |
| eBPF XDP (本実装) | NIC→eBPFマップ→ユーザーバッファ (1回) |

## ビルド

```bash
# libbpf と clang が必要
sudo apt-get install libbpf-dev clang llvm linux-headers-$(uname -r)

# コンパイル
clang -O2 -target bpf -D__TARGET_ARCH_x86 \
  -I/usr/include/$(uname -m)-linux-gnu \
  -c xdp_osc_filter.bpf.c \
  -o xdp_osc_filter.bpf.o
```

## ロード

```bash
# NIC にアタッチ (eth0 は環境に合わせて変更)
sudo ip link set dev eth0 xdp obj xdp_osc_filter.bpf.o sec xdp

# 統計確認
sudo bpftool map dump name packet_stats

# デタッチ
sudo ip link set dev eth0 xdp off
```

## 統計マップ

| キー | 意味 |
|---|---|
| 0 | 総パケット数 |
| 1 | VRChat OSCパケット数 |
| 2 | フィルタ済みパケット数 |
| 3 | パス (通過) パケット数 |

## 要件

- Linux kernel 5.8+
- libbpf 0.5+
- clang/llvm 10+
