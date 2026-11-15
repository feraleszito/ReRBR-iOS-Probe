#![cfg(feature = "harwood-neighbors")]
use rerbr_probe::harwood::neighbors::{self, Budget};
use rerbr_render_core::Vec3;
use std::collections::BTreeSet;

#[test]
#[ignore = "requires private Maps; explicitly set HARWOOD_MAPS"]
fn real_connected_region_has_valid_geometry_shared_textures_and_camera() {
    let maps = std::env::var_os("HARWOOD_MAPS").expect("set HARWOOD_MAPS");
    let set = neighbors::load(std::path::Path::new(&maps)).unwrap();
    let r = &set.report;
    assert!((15..=25).contains(&r.ids.len()), "{r}");
    assert_eq!(r.ids[0], 269);
    assert_eq!(r.ids.len(), r.ids.iter().collect::<BTreeSet<_>>().len());
    assert_eq!(r.connections.len(), r.ids.len() - 1);
    let mut connected = BTreeSet::from([r.ids[0]]);
    for link in &r.connections {
        assert!(connected.contains(&link.from));
        assert!(link.shared_edges > 0 && link.max_gap_m <= 0.02);
        assert!(connected.insert(link.to));
    }
    assert_eq!(connected, r.ids.iter().copied().collect());
    assert_eq!(r.route_runs_m.len(), 1, "{r}");
    assert_eq!(r.covered_samples, r.route_samples);
    assert!(r.route_runs_m[0].1 - r.route_runs_m[0].0 > 400.0);
    assert!(r.draws <= Budget::default().max_draws);
    assert!(r.mesh_bytes <= Budget::default().max_mesh_bytes);
    assert!(r.texture_bytes <= Budget::default().max_texture_bytes);
    assert!(r.texture_bindings > r.unique_textures);
    let info: Vec<_> = set.render.texture_info().collect();
    assert_eq!(info.len(), r.unique_textures);
    assert_eq!(
        info.iter()
            .map(|(_, t)| &t.resolved)
            .collect::<BTreeSet<_>>()
            .len(),
        info.len()
    );
    let texture_ids: BTreeSet<_> = info.iter().map(|&(id, _)| id).collect();
    for draw in &set.render.mesh.draws {
        assert!(draw.indices.end <= set.render.mesh.indices.len());
        for id in draw.textures.into_iter().flatten() {
            assert!(texture_ids.contains(&id));
        }
    }
    for aspect in [0.65, 0.75, 1.55, 2.0] {
        let vp = set.render.camera.view_projection(aspect);
        for &i in &set.render.mesh.indices {
            let v = &set.render.mesh.vertices[i as usize];
            let p = vp * Vec3::from(v.position).extend(1.0);
            let q = p.truncate() / p.w;
            assert!(p.w > 0.0 && q.x.abs() < 1.0 && q.y.abs() < 1.0 && q.z > 0.0 && q.z < 1.0);
        }
    }
}
