//! Allocation probes for the real JSON/frame decoder path.

use figma_dev_mcp_protocol::{
    domain::{DesignNode, MinimalNodeDetails, NodeForest},
    limits::MAX_RETURNED_NODES,
    rpc::decode_frame,
    wire::{BrokerToPlugin, PluginToBroker},
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

struct TrackingAllocator;

thread_local! {
    static TRACKING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATED: Cell<usize> = const { Cell::new(0) };
    static LIVE: Cell<usize> = const { Cell::new(0) };
    static PEAK: Cell<usize> = const { Cell::new(0) };
}

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: Delegates the exact layout to the system allocator.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record_allocation(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        record_deallocation(layout.size());
        // SAFETY: `pointer` came from the system allocator with this layout.
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: Delegates the original allocation and requested size unchanged.
        let replacement = unsafe { System.realloc(pointer, layout, new_size) };
        if !replacement.is_null() {
            record_deallocation(layout.size());
            record_allocation(new_size);
        }
        replacement
    }
}

fn record_allocation(size: usize) {
    let _ = TRACKING.try_with(|tracking| {
        if tracking.get() {
            let _ = ALLOCATED.try_with(|allocated| {
                allocated.set(allocated.get().saturating_add(size));
            });
            let _ = LIVE.try_with(|live| {
                let next = live.get().saturating_add(size);
                live.set(next);
                let _ = PEAK.try_with(|peak| peak.set(peak.get().max(next)));
            });
        }
    });
}

fn record_deallocation(size: usize) {
    let _ = TRACKING.try_with(|tracking| {
        if tracking.get() {
            let _ = LIVE.try_with(|live| live.set(live.get().saturating_sub(size)));
        }
    });
}

fn measure_allocations<T>(operation: impl FnOnce() -> T) -> (T, usize, usize) {
    ALLOCATED.with(|allocated| allocated.set(0));
    LIVE.with(|live| live.set(0));
    PEAK.with(|peak| peak.set(0));
    TRACKING.with(|tracking| tracking.set(true));
    let result = operation();
    TRACKING.with(|tracking| tracking.set(false));
    let allocated = ALLOCATED.with(Cell::get);
    let peak = PEAK.with(Cell::get);
    (result, allocated, peak)
}

#[test]
fn actual_frame_decoder_stops_large_node_id_input_without_content_expansion() {
    let node_ids = std::iter::repeat_n(r#""1:2""#, 50_000)
        .collect::<Vec<_>>()
        .join(",");
    let json = format!(
        r#"{{"requestId":"plugin-1","deadlineMs":100,"target":{{}},"operation":{{"input":{{"nodeIds":[{node_ids}]}},"operation":"get_nodes"}},"type":"request"}}"#
    );
    let mut frame = Vec::with_capacity(json.len() + 4);
    frame.extend_from_slice(&(json.len() as u32).to_be_bytes());
    frame.extend_from_slice(json.as_bytes());

    let (decoded, allocated, peak) = measure_allocations(|| decode_frame::<BrokerToPlugin>(&frame));
    eprintln!("bounded request decode: total={allocated} peak={peak}");
    assert!(decoded.is_err());
    assert!(
        allocated < 1_500_000,
        "actual request decoder allocated {allocated} bytes (peak {peak})"
    );
    assert!(
        peak < 1_000_000,
        "actual request decoder peaked at {peak} live bytes"
    );
}

#[test]
fn actual_plugin_result_decoder_bounds_wide_and_deep_tag_last_payloads() {
    let node = r#"{"summary":{"id":"1:2","name":"Card","nodeType":"FRAME","visible":true,"childIds":[]},"data":{},"children":[],"childrenTruncated":false}"#;
    let nodes = std::iter::repeat_n(node, 5_000)
        .collect::<Vec<_>>()
        .join(",");
    let wide = format!(
        r#"{{"requestId":"plugin-1","result":{{"result":{{"nodes":[{nodes}],"truncated":false,"observation":{{"startedAt":"s","completedAt":"e"}},"detail":"minimal"}},"operation":"get_selection"}},"type":"response"}}"#
    );
    let (decoded, allocated, peak) =
        measure_allocations(|| serde_json::from_str::<PluginToBroker>(&wide));
    eprintln!("bounded result decode: total={allocated} peak={peak}");
    assert!(decoded.is_err());
    assert!(
        allocated < 6_000_000,
        "actual result decoder allocated {allocated} bytes (peak {peak})"
    );
    assert!(
        peak < 4_000_000,
        "actual result decoder peaked at {peak} live bytes"
    );

    let mut nested = node.to_owned();
    for _ in 0..50 {
        nested = format!(
            r#"{{"summary":{{"id":"1:2","name":"Card","nodeType":"FRAME","visible":true,"childIds":[]}},"data":{{}},"children":[{nested}],"childrenTruncated":false}}"#
        );
    }
    let deep = format!(
        r#"{{"requestId":"plugin-1","result":{{"result":{{"nodes":[{nested}],"truncated":false,"observation":{{"startedAt":"s","completedAt":"e"}},"detail":"minimal"}},"operation":"get_selection"}},"type":"response"}}"#
    );
    assert!(serde_json::from_str::<PluginToBroker>(&deep).is_err());
}

#[test]
fn outbound_validation_rejects_width_before_allocating_a_wide_auxiliary_stack() {
    let leaf_json = r#"{
        "summary":{"id":"1:2","name":"Card","nodeType":"FRAME","visible":true,"childIds":[]},
        "data":{},"children":[],"childrenTruncated":false
    }"#;
    let leaf: DesignNode<MinimalNodeDetails> = serde_json::from_str(leaf_json).unwrap();

    let roots = vec![leaf.clone(); MAX_RETURNED_NODES + 1];
    let (result, allocated, peak) = measure_allocations(|| NodeForest::try_from(roots));
    eprintln!("wide-root validation: total={allocated} peak={peak}");
    assert!(result.is_err());
    assert!(
        allocated < 1_024,
        "wide-root validation allocated {allocated} bytes (peak {peak})"
    );

    let mut root = leaf.clone();
    root.children = vec![leaf; MAX_RETURNED_NODES + 1];
    let (result, allocated, peak) = measure_allocations(|| NodeForest::try_from(vec![root]));
    eprintln!("wide-child validation: total={allocated} peak={peak}");
    assert!(result.is_err());
    assert!(
        allocated < 2_048,
        "wide-child validation allocated {allocated} bytes (peak {peak})"
    );
}
