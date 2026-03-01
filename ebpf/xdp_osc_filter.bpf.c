// CR-Bridge eBPF XDP プローブ
// VRChat OSC パケットをカーネル内で識別してフィルタリングする
//
// コンパイル方法:
//   clang -O2 -target bpf -c xdp_osc_filter.bpf.c -o xdp_osc_filter.bpf.o
//
// ロード方法:
//   ip link set dev eth0 xdp obj xdp_osc_filter.bpf.o sec xdp
//   (またはlibbpfを使ったローダーを使用)

#include <linux/bpf.h>
#include <linux/if_ether.h>
#include <linux/ip.h>
#include <linux/ipv6.h>
#include <linux/udp.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>

// VRChat OSC のデフォルトポート
#define VRCHAT_OSC_PORT_DEFAULT 9001
// CR-Bridge が受信するポート（設定可能）
#define CR_BRIDGE_PORT          57200

// eBPF ペルフイベントマップ（ring buffer）
// ATPエンジンが epoll_wait で非同期受信する
struct {
    __uint(type, BPF_MAP_TYPE_PERF_EVENT_ARRAY);
    __uint(key_size, sizeof(__u32));
    __uint(value_size, sizeof(__u32));
    __uint(max_entries, 1024);
} osc_events SEC(".maps");

// パケットカウンター（統計用）
struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(max_entries, 4);
    __type(key, __u32);
    __type(value, __u64);
} packet_stats SEC(".maps");

// カウンターキー
#define STAT_TOTAL_PKTS    0
#define STAT_OSC_PKTS      1
#define STAT_FILTERED_PKTS 2
#define STAT_PASS_PKTS     3

// perf_event に送るメタデータ（ユーザーランドに渡す最小情報）
struct osc_event_t {
    __u64 timestamp_ns;  // カーネルタイムスタンプ
    __u32 src_ip;        // 送信元IP
    __u16 src_port;      // 送信元ポート
    __u16 data_len;      // ペイロード長
    __u8  data[256];     // OSC ペイロード先頭256バイト
};

// カウンターをインクリメントするヘルパー
static __always_inline void incr_stat(__u32 key)
{
    __u64 *val = bpf_map_lookup_elem(&packet_stats, &key);
    if (val)
        __sync_fetch_and_add(val, 1);
}

// OSC パケットかどうかを判定する（簡易チェック）
// OSCアドレスは '/' で始まる文字列
static __always_inline int is_osc_packet(void *data, void *data_end)
{
    if (data + 1 > data_end)
        return 0;
    return *((__u8 *)data) == '/';
}

// VRChat OSC アドレスパターンかどうか確認
// /avatar/parameters/* または /tracking/* を対象とする
static __always_inline int is_vrchat_osc(void *data, void *data_end)
{
    // 最低8バイト必要
    if (data + 8 > data_end)
        return 0;

    __u8 *p = (__u8 *)data;

    // "/avatar" (7文字) チェック
    if (p[0] == '/' && p[1] == 'a' && p[2] == 'v' && p[3] == 'a' &&
        p[4] == 't' && p[5] == 'a' && p[6] == 'r')
        return 1;

    // "/tracking" (9文字) チェック
    if (data + 9 <= data_end &&
        p[0] == '/' && p[1] == 't' && p[2] == 'r' && p[3] == 'a' &&
        p[4] == 'c' && p[5] == 'k' && p[6] == 'i' && p[7] == 'n' && p[8] == 'g')
        return 1;

    return 0;
}

// XDP フック: NICドライバ直後に実行（カーネルネットワークスタック到達前）
// 処理フロー:
//   1. Ethernet ヘッダを解析
//   2. IPv4/IPv6 ヘッダを解析
//   3. UDP ヘッダを解析 → ポート確認
//   4. VRChat OSC パターンを確認
//   5. 一致 → perf_event ring buffer にコピー（1回のみ）
//   6. XDP_PASS → カーネルスタックにも通過（通常受信も継続）
SEC("xdp")
int xdp_osc_filter(struct xdp_md *ctx)
{
    void *data_end = (void *)(long)ctx->data_end;
    void *data     = (void *)(long)ctx->data;

    incr_stat(STAT_TOTAL_PKTS);

    // === Ethernet ヘッダ解析 ===
    struct ethhdr *eth = data;
    if ((void *)(eth + 1) > data_end)
        return XDP_PASS;

    __u16 proto = bpf_ntohs(eth->h_proto);

    void *ip_start = (void *)(eth + 1);
    __u16 udp_dest_port = 0;
    void *udp_payload   = NULL;
    __u16 payload_len   = 0;
    __u32 src_ip        = 0;
    __u16 src_port      = 0;

    // === IPv4 ===
    if (proto == ETH_P_IP) {
        struct iphdr *ip = ip_start;
        if ((void *)(ip + 1) > data_end)
            return XDP_PASS;
        if (ip->protocol != IPPROTO_UDP)
            return XDP_PASS;

        src_ip = ip->saddr;

        struct udphdr *udp = ip_start + (ip->ihl * 4);
        if ((void *)(udp + 1) > data_end)
            return XDP_PASS;

        udp_dest_port = bpf_ntohs(udp->dest);
        src_port      = bpf_ntohs(udp->source);
        udp_payload   = (void *)(udp + 1);
        payload_len   = bpf_ntohs(udp->len) - sizeof(struct udphdr);
    }
    // === IPv6 ===
    else if (proto == ETH_P_IPV6) {
        struct ipv6hdr *ip6 = ip_start;
        if ((void *)(ip6 + 1) > data_end)
            return XDP_PASS;
        if (ip6->nexthdr != IPPROTO_UDP)
            return XDP_PASS;

        struct udphdr *udp = (void *)(ip6 + 1);
        if ((void *)(udp + 1) > data_end)
            return XDP_PASS;

        udp_dest_port = bpf_ntohs(udp->dest);
        src_port      = bpf_ntohs(udp->source);
        udp_payload   = (void *)(udp + 1);
        payload_len   = bpf_ntohs(udp->len) - sizeof(struct udphdr);
    } else {
        return XDP_PASS;
    }

    // === ポートフィルタ ===
    if (udp_dest_port != VRCHAT_OSC_PORT_DEFAULT &&
        udp_dest_port != CR_BRIDGE_PORT) {
        return XDP_PASS;
    }

    // === OSC ペイロード確認 ===
    if (udp_payload == NULL || (void *)((char *)udp_payload + 1) > data_end)
        return XDP_PASS;

    if (!is_osc_packet(udp_payload, data_end))
        return XDP_PASS;

    if (!is_vrchat_osc(udp_payload, data_end)) {
        incr_stat(STAT_PASS_PKTS);
        return XDP_PASS;
    }

    // === VRChat OSC パケット確定 ===
    // perf_event ring buffer にコピー（ユーザーランドへの唯一のコピー）
    incr_stat(STAT_OSC_PKTS);

    struct osc_event_t event = {};
    event.timestamp_ns = bpf_ktime_get_ns();
    event.src_ip       = src_ip;
    event.src_port     = src_port;
    event.data_len     = payload_len < 256 ? payload_len : 256;

    // ペイロード先頭256バイトを ring buffer にコピー
    __u32 copy_len = event.data_len;
    if (copy_len > 0 && copy_len <= 256) {
        if ((void *)((char *)udp_payload + copy_len) <= data_end) {
            bpf_probe_read_kernel(event.data, copy_len, udp_payload);
        }
    }

    // perf_event に書き込み（ATPエンジンが epoll で受信）
    bpf_perf_event_output(ctx, &osc_events,
                          BPF_F_CURRENT_CPU,
                          &event, sizeof(event));

    incr_stat(STAT_FILTERED_PKTS);

    // XDP_PASS: カーネルスタックにも通す（通常のソケット受信も継続）
    return XDP_PASS;
}

char _license[] SEC("license") = "GPL";
