use glam::{IVec3, Mat4, Vec3};
use goblock_render::{push_line, push_rect, HudVertex};
use goblock_world::{BlockId, Face};

/// State of a block currently being targeted or dug.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MiningTarget {
    pub pos: IVec3,
    pub face: Face,
    pub block: BlockId,
    pub progress: f32,
}

/// A flying debris crumb spawned when a block is damaged or broken.
#[derive(Clone, Copy, Debug)]
pub struct Particle {
    pub pos: Vec3,
    pub vel: Vec3,
    pub size: f32,
    pub color: [u8; 4],
    pub life: f32,
    pub max_life: f32,
}

pub struct EffectsSystem {
    pub target: Option<MiningTarget>,
    pub particles: Vec<Particle>,
    chip_timer: f32,
    rng: u64,
}

impl EffectsSystem {
    pub fn new() -> Self {
        Self {
            target: None,
            particles: Vec::with_capacity(128),
            chip_timer: 0.0,
            rng: 0x1234_5678_9abc_def0,
        }
    }

    fn rand_f32(&mut self) -> f32 {
        self.rng = self.rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((self.rng >> 32) as u32 as f32) / (u32::MAX as f32)
    }

    fn rand_range(&mut self, min: f32, max: f32) -> f32 {
        min + self.rand_f32() * (max - min)
    }

    /// Advance active particles and chipping timers.
    pub fn update(&mut self, dt: f32, is_solid: impl Fn(Vec3) -> bool) {
        if let Some(target) = self.target {
            if target.progress > 0.0 && target.progress < 1.0 {
                self.chip_timer += dt;
                if self.chip_timer >= 0.06 {
                    self.chip_timer = 0.0;
                    self.spawn_chips(target.pos, target.face, target.block, 2);
                }
            } else {
                self.chip_timer = 0.0;
            }
        } else {
            self.chip_timer = 0.0;
        }

        const GRAVITY: f32 = 24.0;
        for p in &mut self.particles {
            p.vel.y -= GRAVITY * dt;
            let next_pos = p.pos + p.vel * dt;
            if is_solid(next_pos) {
                p.vel.y = -p.vel.y * 0.35;
                p.vel.x *= 0.6;
                p.vel.z *= 0.6;
            } else {
                p.pos = next_pos;
            }
            p.life -= dt;
        }

        self.particles.retain(|p| p.life > 0.0);
    }

    /// Spawn small chipping crumbs while hitting a block.
    pub fn spawn_chips(&mut self, pos: IVec3, face: Face, block: BlockId, count: usize) {
        let base_color = block.particle_color();
        if base_color[3] == 0 {
            return;
        }

        let normal = face.normal().as_vec3();
        let center = pos.as_vec3() + Vec3::splat(0.5) + normal * 0.52;

        for _ in 0..count {
            let offset = Vec3::new(
                self.rand_range(-0.35, 0.35),
                self.rand_range(-0.35, 0.35),
                self.rand_range(-0.35, 0.35),
            );
            let vel = normal * self.rand_range(1.0, 2.5)
                + Vec3::new(
                    self.rand_range(-1.2, 1.2),
                    self.rand_range(0.8, 2.5),
                    self.rand_range(-1.2, 1.2),
                );
            let tint_variation = self.rand_range(0.85, 1.15);
            let color = [
                ((base_color[0] as f32 * tint_variation).clamp(0.0, 255.0)) as u8,
                ((base_color[1] as f32 * tint_variation).clamp(0.0, 255.0)) as u8,
                ((base_color[2] as f32 * tint_variation).clamp(0.0, 255.0)) as u8,
                240,
            ];
            let size = self.rand_range(0.06, 0.10);
            let life = self.rand_range(0.3, 0.5);

            self.particles.push(Particle {
                pos: center + offset,
                vel,
                size,
                color,
                life,
                max_life: 0.5,
            });
        }
    }

    /// Spawn a full debris burst when a block is destroyed.
    pub fn spawn_break_burst(&mut self, pos: IVec3, block: BlockId) {
        let base_color = block.particle_color();
        if base_color[3] == 0 {
            return;
        }

        let center = pos.as_vec3() + Vec3::splat(0.5);
        for _ in 0..20 {
            let offset = Vec3::new(
                self.rand_range(-0.4, 0.4),
                self.rand_range(-0.4, 0.4),
                self.rand_range(-0.4, 0.4),
            );
            let vel = Vec3::new(
                self.rand_range(-3.0, 3.0),
                self.rand_range(2.0, 5.5),
                self.rand_range(-3.0, 3.0),
            );
            let tint = self.rand_range(0.8, 1.2);
            let color = [
                ((base_color[0] as f32 * tint).clamp(0.0, 255.0)) as u8,
                ((base_color[1] as f32 * tint).clamp(0.0, 255.0)) as u8,
                ((base_color[2] as f32 * tint).clamp(0.0, 255.0)) as u8,
                255,
            ];

            let life = self.rand_range(0.45, 0.75);
            let size = self.rand_range(0.08, 0.14);
            self.particles.push(Particle {
                pos: center + offset,
                vel,
                size,
                color,
                life,
                max_life: life,
            });
        }
    }

    /// Render in-world crack overlay and flying debris particles to `out`.
    pub fn render(
        &self,
        out: &mut Vec<HudVertex>,
        camera_pos: Vec3,
        view_proj: Mat4,
        screen_w: f32,
        screen_h: f32,
    ) {
        if let Some(target) = self.target {
            render_block_overlay(out, target, camera_pos, view_proj, screen_w, screen_h);
        }

        for p in &self.particles {
            let dist = (p.pos - camera_pos).length();
            if !(0.2..=32.0).contains(&dist) {
                continue;
            }

            let Some(screen) = project(p.pos, camera_pos, view_proj, screen_w, screen_h) else {
                continue;
            };

            let radius = ((p.size * 0.5 / dist) * screen_h * 0.55).clamp(1.5, 10.0);
            let alpha_factor = (p.life / p.max_life).clamp(0.0, 1.0);
            let alpha = (p.color[3] as f32 * alpha_factor) as u8;
            if alpha < 5 {
                continue;
            }

            let shadow = [0, 0, 0, (alpha as f32 * 0.5) as u8];
            let fill = [p.color[0], p.color[1], p.color[2], alpha];

            push_rect(
                out,
                screen[0] - radius - 0.5,
                screen[1] - radius - 0.5,
                radius * 2.0 + 1.0,
                radius * 2.0 + 1.0,
                shadow,
            );
            push_rect(
                out,
                screen[0] - radius,
                screen[1] - radius,
                radius * 2.0,
                radius * 2.0,
                fill,
            );
        }
    }
}

impl Default for EffectsSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Project a 3D world coordinate into 2D UI screen coordinates.
pub fn project(
    p: Vec3,
    camera_pos: Vec3,
    view_proj: Mat4,
    screen_w: f32,
    screen_h: f32,
) -> Option<[f32; 2]> {
    let rel_p = p - camera_pos;
    let clip = view_proj * rel_p.extend(1.0);
    if clip.w <= 0.1 {
        return None;
    }
    let ndc_x = clip.x / clip.w;
    let ndc_y = clip.y / clip.w;
    let ndc_z = clip.z / clip.w;
    if !(0.0..=1.0).contains(&ndc_z)
        || !(-1.5..=1.5).contains(&ndc_x)
        || !(-1.5..=1.5).contains(&ndc_y)
    {
        return None;
    }
    let sx = (ndc_x * 0.5 + 0.5) * screen_w;
    let sy = (ndc_y * 0.5 + 0.5) * screen_h;
    Some([sx, sy])
}

fn render_block_overlay(
    out: &mut Vec<HudVertex>,
    target: MiningTarget,
    camera_pos: Vec3,
    view_proj: Mat4,
    screen_w: f32,
    screen_h: f32,
) {
    let b = target.pos;
    // Slightly expand the cube by 0.002 to avoid z-fighting with terrain.
    let min = b.as_vec3() - Vec3::splat(0.002);
    let max = (b + IVec3::ONE).as_vec3() + Vec3::splat(0.002);

    let corners_3d = [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(max.x, max.y, max.z),
        Vec3::new(min.x, max.y, max.z),
    ];

    let mut projected = [None; 8];
    for (i, p) in corners_3d.iter().enumerate() {
        projected[i] = project(*p, camera_pos, view_proj, screen_w, screen_h);
    }

    const EDGES: [(usize, usize); 12] = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];

    // Highlight bounding outline
    let outline_alpha = if target.progress > 0.0 { 180 } else { 120 };
    let outline_color = [15, 15, 15, outline_alpha];
    for &(i0, i1) in &EDGES {
        if let (Some(p0), Some(p1)) = (projected[i0], projected[i1]) {
            push_line(out, p0, p1, 1.5, outline_color);
        }
    }

    // When digging, render crack lines across the hit face
    if target.progress > 0.0 {
        let (f0, f1, f2, f3) = face_corners_3d(min, max, target.face);
        let center = (f0 + f1 + f2 + f3) * 0.25;

        let Some(pc) = project(center, camera_pos, view_proj, screen_w, screen_h) else {
            return;
        };
        let p0 = project(f0, camera_pos, view_proj, screen_w, screen_h);
        let p1 = project(f1, camera_pos, view_proj, screen_w, screen_h);
        let p2 = project(f2, camera_pos, view_proj, screen_w, screen_h);
        let p3 = project(f3, camera_pos, view_proj, screen_w, screen_h);

        let crack_color = [20, 20, 20, 220];
        let crack_highlight = [255, 255, 255, 140];
        let t = 2.0;

        let pairs = [
            (p0, 0.15),
            (p2, 0.30),
            (p1, 0.45),
            (p3, 0.60),
            (
                p0.and_then(|a| p1.map(|b| [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5])),
                0.75,
            ),
            (
                p2.and_then(|a| p3.map(|b| [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5])),
                0.90,
            ),
        ];

        for (pt_opt, threshold) in pairs {
            if target.progress >= threshold {
                if let Some(target_pt) = pt_opt {
                    push_line(out, pc, target_pt, t + 0.5, crack_color);
                    push_line(out, pc, target_pt, t * 0.5, crack_highlight);
                }
            }
        }
    }
}

fn face_corners_3d(min: Vec3, max: Vec3, face: Face) -> (Vec3, Vec3, Vec3, Vec3) {
    match face {
        Face::Up => (
            Vec3::new(min.x, max.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(max.x, max.y, max.z),
            Vec3::new(min.x, max.y, max.z),
        ),
        Face::Down => (
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(min.x, min.y, max.z),
        ),
        Face::North => (
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(min.x, max.y, min.z),
        ),
        Face::South => (
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(max.x, max.y, max.z),
            Vec3::new(min.x, max.y, max.z),
        ),
        Face::West => (
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(min.x, max.y, max.z),
            Vec3::new(min.x, max.y, min.z),
        ),
        Face::East => (
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(max.x, max.y, max.z),
            Vec3::new(max.x, max.y, min.z),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_system_lifecycle() {
        let mut effects = EffectsSystem::new();
        assert_eq!(effects.particles.len(), 0);

        effects.spawn_break_burst(IVec3::new(0, 10, 0), BlockId::Stone);
        assert_eq!(effects.particles.len(), 20);

        effects.update(1.0, |_| false);
        assert_eq!(effects.particles.len(), 0);
    }

    #[test]
    fn projection_returns_coords_for_point_in_front_of_camera() {
        let camera = Vec3::new(0.0, 0.0, 5.0);
        let view = Mat4::look_to_rh(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0), Vec3::Y);
        let proj = Mat4::perspective_rh(60.0f32.to_radians(), 1.0, 0.1, 100.0);
        let flip_y = Mat4::from_scale(Vec3::new(1.0, -1.0, 1.0));
        let view_proj = flip_y * proj * view;

        let point = Vec3::new(0.0, 0.0, 0.0);
        let res = project(point, camera, view_proj, 800.0, 600.0);
        assert!(res.is_some());
        let [x, y] = res.unwrap();
        assert!((x - 400.0).abs() < 2.0);
        assert!((y - 300.0).abs() < 2.0);
    }

    #[test]
    fn projection_returns_none_for_point_behind_camera() {
        let camera = Vec3::new(0.0, 0.0, 5.0);
        let view = Mat4::look_to_rh(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0), Vec3::Y);
        let proj = Mat4::perspective_rh(60.0f32.to_radians(), 1.0, 0.1, 100.0);
        let flip_y = Mat4::from_scale(Vec3::new(1.0, -1.0, 1.0));
        let view_proj = flip_y * proj * view;

        let point = Vec3::new(0.0, 0.0, 10.0);
        let res = project(point, camera, view_proj, 800.0, 600.0);
        assert!(res.is_none());
    }
}
