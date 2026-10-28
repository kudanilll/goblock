//! The loaded part of an endless world. Chunks around a centre point are
//! generated and meshed on worker threads, and dropped again once the centre
//! moves away.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use glam::{IVec2, IVec3, Vec3};

use crate::biome::Biome;
use crate::block::BlockId;
use crate::chunk::{chunk_of, Chunk, CHUNK_SIZE};
use crate::generate::generate_chunk;
use crate::mesh::{build_mesh, ChunkMesh};
use crate::terrain::Terrain;

/// What changed since the last [`World::update`].
#[derive(Debug)]
pub enum WorldEvent {
    /// A chunk's mesh is ready to upload. A chunk is meshed once while it
    /// stays loaded.
    Meshed(IVec2, ChunkMesh),
    /// A meshed chunk left the view range and its mesh can be freed.
    Unloaded(IVec2),
}

enum Job {
    Generate(IVec2),
    Mesh(IVec2, Box<[Arc<Chunk>; 9]>),
}

enum Done {
    Generated(Chunk),
    Meshed(IVec2, ChunkMesh),
}

pub struct World {
    terrain: Arc<Terrain>,
    view_distance: i32,
    /// Offsets of every chunk to generate, nearest first. They reach two
    /// chunks past the view distance, so every chunk that gets meshed has
    /// all eight neighbours, diagonals included.
    offsets: Vec<IVec2>,
    chunks: HashMap<IVec2, Arc<Chunk>>,
    meshed: HashSet<IVec2>,
    generating: HashSet<IVec2>,
    meshing: HashSet<IVec2>,
    max_in_flight: usize,
    jobs: Option<Sender<Job>>,
    done: Receiver<Done>,
    workers: Vec<JoinHandle<()>>,
}

impl World {
    /// Start a world for `seed` that keeps chunks meshed up to
    /// `view_distance` chunks from the centre.
    pub fn new(seed: u64, view_distance: i32) -> std::io::Result<Self> {
        let terrain = Arc::new(Terrain::new(seed));
        let (job_sender, job_receiver) = mpsc::channel();
        let (done_sender, done) = mpsc::channel();
        let job_receiver = Arc::new(Mutex::new(job_receiver));

        // Leave a core free for the render thread.
        let count = thread::available_parallelism()
            .map_or(2, |n| n.get())
            .saturating_sub(1)
            .clamp(1, 6);
        let workers = (0..count)
            .map(|i| {
                let jobs = Arc::clone(&job_receiver);
                let done = done_sender.clone();
                let terrain = Arc::clone(&terrain);
                thread::Builder::new()
                    .name(format!("world-{i}"))
                    .spawn(move || work(&terrain, &jobs, &done))
            })
            .collect::<std::io::Result<Vec<_>>>()?;

        let reach = view_distance + 2;
        let mut offsets: Vec<IVec2> = (-reach..=reach)
            .flat_map(|z| (-reach..=reach).map(move |x| IVec2::new(x, z)))
            .filter(|o| o.length_squared() <= reach * reach)
            .collect();
        offsets.sort_by_key(|o| o.length_squared());

        Ok(Self {
            terrain,
            view_distance,
            offsets,
            chunks: HashMap::new(),
            meshed: HashSet::new(),
            generating: HashSet::new(),
            meshing: HashSet::new(),
            max_in_flight: count * 2,
            jobs: Some(job_sender),
            done,
            workers,
        })
    }

    pub fn terrain(&self) -> &Terrain {
        &self.terrain
    }

    /// Stream chunks around `centre`. Call once per frame.
    pub fn update(&mut self, centre: Vec3) -> Vec<WorldEvent> {
        let centre = chunk_of(centre.x.floor() as i32, centre.z.floor() as i32);
        let mut events = Vec::new();
        self.collect(centre, &mut events);
        self.unload(centre, &mut events);
        self.queue(centre);
        events
    }

    pub fn block_at(&self, p: IVec3) -> BlockId {
        let pos = chunk_of(p.x, p.z);
        match self.chunks.get(&pos) {
            Some(chunk) => chunk.get(p.x - pos.x * CHUNK_SIZE, p.y, p.z - pos.y * CHUNK_SIZE),
            None => BlockId::Air,
        }
    }

    /// Whether `point` is inside a solid block. Chunks that are not loaded
    /// count as empty.
    pub fn is_solid(&self, point: Vec3) -> bool {
        self.block_at(point.floor().as_ivec3()).is_solid()
    }

    pub fn biome_at(&self, x: i32, z: i32) -> Biome {
        self.terrain.sample(x, z).biome
    }

    /// Loaded chunks and how many of them are meshed.
    pub fn stats(&self) -> (usize, usize) {
        (self.chunks.len(), self.meshed.len())
    }

    fn collect(&mut self, centre: IVec2, events: &mut Vec<WorldEvent>) {
        let keep = self.view_distance + 3;
        while let Ok(done) = self.done.try_recv() {
            match done {
                Done::Generated(chunk) => {
                    let pos = chunk.pos();
                    self.generating.remove(&pos);
                    if within(pos, centre, keep) {
                        self.chunks.insert(pos, Arc::new(chunk));
                    }
                }
                Done::Meshed(pos, mesh) => {
                    self.meshing.remove(&pos);
                    let wanted = within(pos, centre, self.view_distance + 1);
                    if wanted && self.chunks.contains_key(&pos) {
                        self.meshed.insert(pos);
                        events.push(WorldEvent::Meshed(pos, mesh));
                    }
                }
            }
        }
    }

    fn unload(&mut self, centre: IVec2, events: &mut Vec<WorldEvent>) {
        let mesh_range = self.view_distance + 1;
        self.meshed.retain(|&pos| {
            let keep = within(pos, centre, mesh_range);
            if !keep {
                events.push(WorldEvent::Unloaded(pos));
            }
            keep
        });
        // One chunk past the generation range, so walking back and forth
        // over a border does not regenerate the same chunks.
        let keep = self.view_distance + 3;
        self.chunks.retain(|&pos, _| within(pos, centre, keep));
    }

    fn queue(&mut self, centre: IVec2) {
        let Some(jobs) = &self.jobs else {
            return;
        };
        let mut budget = self
            .max_in_flight
            .saturating_sub(self.generating.len() + self.meshing.len());

        for offset in &self.offsets {
            if budget == 0 {
                break;
            }
            let pos = centre + *offset;

            if !self.chunks.contains_key(&pos) {
                if !self.generating.contains(&pos) && jobs.send(Job::Generate(pos)).is_ok() {
                    self.generating.insert(pos);
                    budget -= 1;
                }
                continue;
            }

            let wants_mesh = offset.length_squared() <= self.view_distance * self.view_distance
                && !self.meshed.contains(&pos)
                && !self.meshing.contains(&pos);
            if wants_mesh {
                if let Some(area) = neighbourhood(&self.chunks, pos) {
                    if jobs.send(Job::Mesh(pos, area)).is_ok() {
                        self.meshing.insert(pos);
                        budget -= 1;
                    }
                }
            }
        }
    }
}

impl Drop for World {
    fn drop(&mut self) {
        // Closing the job channel lets every worker finish and exit.
        self.jobs = None;
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

fn within(pos: IVec2, centre: IVec2, radius: i32) -> bool {
    (pos - centre).length_squared() <= radius * radius
}

/// `pos` and its eight neighbours, if all of them are loaded.
fn neighbourhood(chunks: &HashMap<IVec2, Arc<Chunk>>, pos: IVec2) -> Option<Box<[Arc<Chunk>; 9]>> {
    let mut area = Vec::with_capacity(9);
    for dz in -1..=1 {
        for dx in -1..=1 {
            area.push(Arc::clone(chunks.get(&(pos + IVec2::new(dx, dz)))?));
        }
    }
    area.try_into().ok().map(Box::new)
}

fn work(terrain: &Terrain, jobs: &Mutex<Receiver<Job>>, done: &Sender<Done>) {
    loop {
        let job = match jobs.lock() {
            Ok(receiver) => receiver.recv(),
            Err(_) => return,
        };
        let Ok(job) = job else {
            return;
        };
        let result = match job {
            Job::Generate(pos) => Done::Generated(generate_chunk(terrain, pos)),
            Job::Mesh(pos, area) => {
                let chunks = std::array::from_fn(|i| &*area[i]);
                Done::Meshed(pos, build_mesh(chunks))
            }
        };
        if done.send(result).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// Update until `done` holds or the deadline passes, collecting events.
    fn pump(world: &mut World, centre: Vec3, done: impl Fn(&World) -> bool) -> Vec<WorldEvent> {
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut events = Vec::new();
        while !done(world) {
            assert!(Instant::now() < deadline, "world did not settle");
            events.extend(world.update(centre));
            thread::sleep(Duration::from_millis(2));
        }
        events
    }

    fn chunks_within(radius: i32) -> usize {
        (-radius..=radius)
            .flat_map(|z| (-radius..=radius).map(move |x| IVec2::new(x, z)))
            .filter(|o| o.length_squared() <= radius * radius)
            .count()
    }

    #[test]
    fn chunks_around_the_centre_get_meshed() {
        let mut world = World::new(1, 2).unwrap();
        let expected = chunks_within(2);
        let events = pump(&mut world, Vec3::ZERO, |w| w.stats().1 == expected);
        let meshed = events
            .iter()
            .filter(|e| matches!(e, WorldEvent::Meshed(..)))
            .count();
        assert_eq!(meshed, expected);
    }

    #[test]
    fn moving_away_unloads_old_chunks() {
        let mut world = World::new(1, 1).unwrap();
        let expected = chunks_within(1);
        pump(&mut world, Vec3::ZERO, |w| w.stats().1 == expected);

        let events = world.update(Vec3::new(10_000.0, 0.0, 0.0));
        let unloaded = events
            .iter()
            .filter(|e| matches!(e, WorldEvent::Unloaded(..)))
            .count();
        assert_eq!(unloaded, expected);
        assert_eq!(world.stats(), (0, 0));
    }

    #[test]
    fn loaded_ground_is_solid() {
        let mut world = World::new(4, 1).unwrap();
        let (x, z) = world.terrain().find_spawn();
        let centre = Vec3::new(x as f32, 0.0, z as f32);
        pump(&mut world, centre, |w| w.stats().1 == chunks_within(1));

        let height = world.terrain().sample(x, z).height;
        let (fx, fz) = (x as f32 + 0.5, z as f32 + 0.5);
        assert!(world.is_solid(Vec3::new(fx, height as f32 + 0.5, fz)));
        assert!(!world.is_solid(Vec3::new(fx, 380.5, fz)));
    }

    #[test]
    fn unloaded_space_is_empty() {
        let world = World::new(1, 1).unwrap();
        assert_eq!(world.block_at(IVec3::new(0, 10, 0)), BlockId::Air);
    }
}
