//! Measures world streaming throughput, chunk counts, vertex memory and greedy
//! meshing estimates across view distances without needing a GPU.

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::time::{Duration, Instant};

use glam::{IVec2, Vec3};
use goblock_world::{ChunkMesh, ChunkVertex, World, WorldEvent};

struct Counting;
static LIVE: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = System.alloc(layout);
        if !p.is_null() {
            LIVE.fetch_add(layout.size(), Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        System.dealloc(p, layout);
        LIVE.fetch_sub(layout.size(), Relaxed);
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

const FRAME: Duration = Duration::from_millis(16);
const MIB: f64 = 1024.0 * 1024.0;

/// Quads left if flat-shaded coplanar neighbours with the same texture and tint
/// were merged. Quads with an AO gradient stay single.
fn greedy(quads: &[ChunkVertex]) -> usize {
    type Key = (u16, usize, u16, u8, u8, [u8; 4]);
    let mut groups: HashMap<Key, HashSet<(u16, u16)>> = HashMap::new();
    let mut kept = 0;
    for quad in quads.chunks_exact(4) {
        let shade = quad[0].uv_layer_shade[3];
        if quad.iter().any(|v| v.uv_layer_shade[3] != shade) {
            kept += 1;
            continue;
        }
        let min = [0, 1, 2].map(|i| quad.iter().map(|v| v.position[i]).min().unwrap());
        let max = [0, 1, 2].map(|i| quad.iter().map(|v| v.position[i]).max().unwrap());
        let Some(axis) = (0..3).find(|&i| min[i] == max[i]) else {
            kept += 1;
            continue;
        };
        let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
        let key = (
            quad[0].position[3],
            axis,
            min[axis],
            quad[0].uv_layer_shade[2],
            shade,
            quad[0].tint,
        );
        groups
            .entry(key)
            .or_default()
            .insert((min[a] / 16, min[b] / 16));
    }
    let mut merged = 0;
    for cells in groups.values_mut() {
        let mut order: Vec<_> = cells.iter().copied().collect();
        order.sort();
        for (u, v) in order {
            if !cells.contains(&(u, v)) {
                continue;
            }
            let mut w = 1;
            while cells.contains(&(u + w, v)) {
                w += 1;
            }
            let mut h = 1;
            'grow: loop {
                for i in 0..w {
                    if !cells.contains(&(u + i, v + h)) {
                        break 'grow;
                    }
                }
                h += 1;
            }
            for i in 0..w {
                for j in 0..h {
                    cells.remove(&(u + i, v + j));
                }
            }
            merged += 1;
        }
    }
    kept + merged
}

#[derive(Default)]
struct Totals {
    meshes: usize,
    empty: usize,
    opaque: u64,
    cutout: u64,
    water: u64,
    max_quads: u32,
    layer_draws: u64,
    greedy_opaque: u64,
    greedy_water: u64,
}

impl Totals {
    fn add(&mut self, mesh: &ChunkMesh) {
        self.meshes += 1;
        if mesh.is_empty() {
            self.empty += 1;
            return;
        }
        self.opaque += mesh.opaque_quads as u64;
        self.cutout += mesh.cutout_quads as u64;
        self.water += mesh.translucent_quads as u64;
        self.max_quads = self.max_quads.max(mesh.quad_count());
        self.layer_draws += [mesh.opaque_quads, mesh.cutout_quads, mesh.translucent_quads]
            .iter()
            .filter(|&&q| q > 0)
            .count() as u64;
        let opaque_end = mesh.opaque_quads as usize * 4;
        let water_start = (mesh.opaque_quads + mesh.cutout_quads) as usize * 4;
        self.greedy_opaque += greedy(&mesh.vertices[..opaque_end]) as u64;
        self.greedy_water += greedy(&mesh.vertices[water_start..]) as u64;
    }
}

fn run(seed: u64, vd: i32, travel: bool) {
    let base = LIVE.load(Relaxed);
    let started = Instant::now();
    let mut world = World::new(seed, vd).expect("world");
    let (x, z) = world.terrain().find_spawn();
    let mut centre = Vec3::new(x as f32 + 0.5, 90.0, z as f32 + 0.5);
    let wanted = (-vd..=vd)
        .flat_map(|z| (-vd..=vd).map(move |x| IVec2::new(x, z)))
        .filter(|o| o.length_squared() <= vd * vd)
        .count();

    let mut totals = Totals::default();
    let mut meshed: HashSet<IVec2> = HashSet::new();
    let mut max_burst = 0;
    while totals.meshes < wanted && started.elapsed() < Duration::from_secs(180) {
        let events = world.update(centre);
        max_burst = max_burst.max(events.len());
        for event in events {
            match event {
                WorldEvent::Meshed(pos, mesh) => {
                    totals.add(&mesh);
                    meshed.insert(pos);
                }
                WorldEvent::Unloaded(pos) => {
                    meshed.remove(&pos);
                }
            }
        }
        std::thread::sleep(FRAME);
    }
    let load = started.elapsed().as_secs_f64();

    let idle_start = Instant::now();
    for _ in 0..2000 {
        let _ = world.update(centre);
    }
    let idle_us = idle_start.elapsed().as_secs_f64() * 1e6 / 2000.0;

    let heap = (LIVE.load(Relaxed) - base) as f64;
    let (loaded, _) = world.stats();
    let quads = totals.opaque + totals.cutout + totals.water;
    let solid = (totals.meshes - totals.empty).max(1) as u64;
    println!(
        "seed={seed} vd={vd} meshed={} empty={} loaded={loaded} load_s={load:.1} chunks_per_s={:.0} max_burst={max_burst}",
        totals.meshes,
        totals.empty,
        totals.meshes as f64 / load,
    );
    println!(
        "  quads total={quads} opaque={} cutout={} water={} mean={} max={}",
        totals.opaque,
        totals.cutout,
        totals.water,
        quads / solid,
        totals.max_quads,
    );
    println!(
        "  vertex_mib={:.1} mean_kib_per_chunk={:.1} index_mib={:.2} heap_mib={:.1} heap_kib_per_loaded_chunk={:.1}",
        (quads * 64) as f64 / MIB,
        (quads * 64) as f64 / solid as f64 / 1024.0,
        (totals.max_quads as u64 * 24) as f64 / MIB,
        heap / MIB,
        heap / loaded as f64 / 1024.0,
    );
    println!(
        "  layer_draws_all={} greedy_opaque_ratio={:.2} greedy_water_ratio={:.2} update_idle_us={idle_us:.0}",
        totals.layer_draws,
        totals.greedy_opaque as f64 / totals.opaque.max(1) as f64,
        totals.greedy_water as f64 / totals.water.max(1) as f64,
    );

    if travel {
        // Fly-sprint along +x and record how close the nearest missing chunk
        // gets. Fog closes at vd - 1.5 chunks, so anything nearer is a hole.
        for speed in [15.0f32, 37.5] {
            let start = Instant::now();
            let mut last = Instant::now();
            let mut clear = f32::MAX;
            while start.elapsed() < Duration::from_secs(15) {
                let now = Instant::now();
                centre.x += speed * (now - last).as_secs_f32();
                last = now;
                for event in world.update(centre) {
                    match event {
                        WorldEvent::Meshed(pos, _) => {
                            meshed.insert(pos);
                        }
                        WorldEvent::Unloaded(pos) => {
                            meshed.remove(&pos);
                        }
                    }
                }
                let here = IVec2::new(
                    (centre.x / 16.0).floor() as i32,
                    (centre.z / 16.0).floor() as i32,
                );
                for z in -vd..=vd {
                    for x in -vd..=vd {
                        let o = IVec2::new(x, z);
                        if o.length_squared() <= vd * vd && !meshed.contains(&(here + o)) {
                            clear = clear.min((o.length_squared() as f32).sqrt());
                        }
                    }
                }
                std::thread::sleep(FRAME);
            }
            println!(
                "  travel speed={speed} nearest_missing_chunks={clear:.1} fog_end_chunks={:.1}",
                vd as f32 - 1.5
            );
        }
    }
}

fn main() {
    println!(
        "workers={}",
        std::thread::available_parallelism()
            .map_or(2, |n| n.get())
            .saturating_sub(1)
            .clamp(1, 6)
    );
    run(1, 16, true);
    run(42, 16, false);
    run(1, 32, true);
}
