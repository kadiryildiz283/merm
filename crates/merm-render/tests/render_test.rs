use merm_render::{BackendType, LodCache, RenderEngine, Transform2D};

#[test]
fn test_transform_pan_and_zoom() {
    let mut t = Transform2D::default();
    t.pan(50.0, 100.0);
    assert_eq!(t.pan_x, 50.0);
    assert_eq!(t.pan_y, 100.0);

    t.zoom_at(2.0, 100.0, 100.0);
    assert_eq!(t.scale, 2.0);

    let (wx, wy) = t.screen_to_world(100.0, 100.0);
    let (sx, sy) = t.world_to_screen(wx, wy);
    assert!((sx - 100.0).abs() < 1e-4);
    assert!((sy - 100.0).abs() < 1e-4);
}

#[test]
fn test_lod_cache_eviction() {
    // 1 MB cache
    let mut cache = LodCache::new(1);

    // Each 512x512 texture is 1MB (512 * 512 * 4 bytes)
    cache.insert(1, 512, 512);
    assert_eq!(cache.len(), 1);

    // Insert second texture, should evict first
    cache.insert(2, 512, 512);
    assert_eq!(cache.len(), 1);
    assert!(cache.get(2).is_some());
    assert!(cache.get(1).is_none());
}

#[test]
fn test_cpu_fallback_renderer() {
    let mut engine = RenderEngine::force_software();
    assert_eq!(engine.active_backend(), BackendType::SoftwareFallback);

    let t = Transform2D::default();
    let res = engine.render(&t, "<svg><g id=\"node_1\"></g></svg>");
    assert_eq!(res.backend, BackendType::SoftwareFallback);
    assert_eq!(res.rendered_objects, 1);
}
