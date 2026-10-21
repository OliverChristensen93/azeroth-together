//! A strike's bolt in the sky: a jagged, forking line of cold light from the clouds down to the
//! ground, out where the strike fell, lit with the flash's own pulses and gone within a second.
//! Drawn once per strike as flat strips turned toward the camera, a wide, faint glow under a
//! thin, bright core; additive and unlit, so it adds light and trees in front cut it.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use benilla_world::view::WorldCamera;

use super::lightning::flash_of_pulses;

/// A bolt to draw: where the strike fell and its flash's pulses.
#[derive(Message, Clone, Debug)]
pub(super) struct BoltSpawn {
    /// Radians about the vertical, as the thunder's (`(cos, 0, sin)`).
    pub bearing: f32,
    /// The thunder's delay, seconds: a far strike's bolt stands farther off.
    pub delay: f32,
    pub pulses: Vec<(f32, f32)>,
    pub seed: u64,
}

/// A bolt being drawn.
#[derive(Component)]
struct Bolt {
    born: f64,
    pulses: Vec<(f32, f32)>,
    material: Handle<StandardMaterial>,
}

/// How long a bolt stays, seconds (its last pulse has faded well before).
const LIFE: f32 = 1.0;
/// The core's half width and the glow's, yards.
const CORE: f32 = 0.35;
const GLOW: f32 = 3.0;
const CORE_COLOR: [f32; 4] = [0.92, 0.95, 1.0, 1.0];
const GLOW_COLOR: [f32; 4] = [0.5, 0.6, 1.0, 0.5];

/// A small deterministic random stream.
struct Rand(u64);

impl Rand {
    fn next(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 40) as f32) / (1u64 << 24) as f32
    }

    fn signed(&mut self) -> f32 {
        self.next() * 2.0 - 1.0
    }
}

/// A jagged line from `top` to `bottom` in `n` segments, each kink a random step sideways that
/// is pulled back toward the straight line.
fn jag(top: Vec3, bottom: Vec3, n: usize, rough: f32, r: &mut Rand) -> Vec<Vec3> {
    let along = bottom - top;
    let side = along.cross(Vec3::Z).try_normalize().unwrap_or(Vec3::X);
    let depth = along.cross(side).try_normalize().unwrap_or(Vec3::Z);
    let seg = along.length() / n as f32;
    let mut off = Vec2::ZERO;
    (0..=n)
        .map(|i| {
            off = if i > 0 && i < n {
                off * 0.55 + Vec2::new(r.signed(), r.signed()) * seg * rough
            } else {
                Vec2::ZERO
            };
            top + along * (i as f32 / n as f32) + side * off.x + depth * off.y
        })
        .collect()
}

/// The bolt's lines with their width scales: the trunk and two or three forks off it.
fn bolt_lines(top: Vec3, bottom: Vec3, seed: u64) -> Vec<(Vec<Vec3>, f32)> {
    let mut r = Rand(seed | 1);
    let trunk = jag(top, bottom, 26, 0.85, &mut r);
    let mut lines = vec![(trunk.clone(), 1.0)];
    let forks = 2 + (r.next() * 2.0) as usize;
    for _ in 0..forks {
        let at = 3 + (r.next() * 15.0) as usize;
        let from = trunk[at];
        let drop = (bottom.y - top.y).abs() * (0.15 + 0.2 * r.next());
        let out = Vec3::new(r.signed(), 0.0, r.signed()).normalize_or_zero() * drop * 0.6;
        let to = from + out - Vec3::Y * drop;
        lines.push((jag(from, to, 9, 0.9, &mut r), 0.5));
    }
    lines
}

/// The mesh of `lines`, each a camera-facing strip seen from `eye`: a glow strip under a core,
/// with its vertices relative to `origin` (the bolt's foot, so it sorts among the see-through
/// draws by where it stands).
fn bolt_mesh(lines: &[(Vec<Vec3>, f32)], eye: Vec3, origin: Vec3) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    for (half, color, edge_alpha) in [(GLOW, GLOW_COLOR, 0.0), (CORE, CORE_COLOR, 1.0)] {
        let edge = [color[0], color[1], color[2], color[3] * edge_alpha];
        for (points, scale) in lines {
            for w in points.windows(2) {
                let (a, b) = (w[0], w[1]);
                let side = (b - a).cross(eye - a).try_normalize().unwrap_or(Vec3::X) * half * scale;
                // Three columns across (edge, middle, edge), so the glow is brightest down its
                // middle.
                let base = positions.len() as u32;
                for p in [a - side, a, a + side, b - side, b, b + side] {
                    positions.push((p - origin).to_array());
                }
                colors.extend([edge, color, edge, edge, color, edge]);
                indices.extend([
                    base, base + 1, base + 4, base, base + 4, base + 3,
                    base + 1, base + 2, base + 5, base + 1, base + 5, base + 4,
                ]);
            }
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(indices))
}

fn spawn(
    mut commands: Commands,
    time: Res<Time>,
    mut spawns: MessageReader<BoltSpawn>,
    cam: Query<&GlobalTransform, With<WorldCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(eye) = cam.iter().next().map(GlobalTransform::translation) else {
        spawns.clear();
        return;
    };
    for s in spawns.read() {
        let mut r = Rand(s.seed ^ 0xB017);
        let dist = 60.0 + 25.0 * s.delay;
        let dir = Vec3::new(s.bearing.cos(), 0.0, s.bearing.sin());
        let foot = eye + dir * dist;
        let top = foot + Vec3::Y * (70.0 + 40.0 * r.next()) + dir * (r.signed() * 8.0);
        let bottom = Vec3::new(foot.x, eye.y - 25.0, foot.z);
        let mesh = bolt_mesh(&bolt_lines(top, bottom, s.seed), eye, foot);
        let material = materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 1.0, 1.0, 0.0),
            unlit: true,
            fog_enabled: false,
            alpha_mode: AlphaMode::Add,
            cull_mode: None,
            ..default()
        });
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(foot),
            Bolt {
                born: time.elapsed_secs_f64(),
                pulses: s.pulses.clone(),
                material,
            },
        ));
    }
}

fn light(
    mut commands: Commands,
    time: Res<Time>,
    bolts: Query<(Entity, &Bolt)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let now = time.elapsed_secs_f64();
    for (e, bolt) in &bolts {
        let t = (now - bolt.born) as f32;
        if t > LIFE {
            materials.remove(&bolt.material);
            commands.entity(e).despawn();
            continue;
        }
        // The bolt holds a little longer than the sky's flash: the eye keeps its image.
        let k = flash_of_pulses(&bolt.pulses, t).max(flash_of_pulses(&bolt.pulses, t * 0.6) * 0.5);
        if let Some(m) = materials.get_mut(&bolt.material) {
            m.base_color = Color::srgba(1.0, 1.0, 1.0, (k * 1.6).min(1.0));
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_message::<BoltSpawn>()
        .add_systems(Update, (spawn, light).chain());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bolt_runs_from_its_top_to_its_foot_and_forks() {
        let top = Vec3::new(0.0, 100.0, 0.0);
        let bottom = Vec3::new(0.0, -20.0, 0.0);
        let lines = bolt_lines(top, bottom, 42);
        assert!(lines.len() >= 3);
        let trunk = &lines[0].0;
        assert_eq!(trunk.first(), Some(&top));
        assert_eq!(trunk.last(), Some(&bottom));
        // Kinks stay near the straight line.
        assert!(trunk.iter().all(|p| p.xz().length() < 40.0));
        // Forks go down from the trunk.
        for (fork, _) in &lines[1..] {
            assert!(fork.last().unwrap().y < fork.first().unwrap().y);
        }
    }

    #[test]
    fn the_mesh_has_two_strips_of_six_per_segment() {
        let lines = bolt_lines(Vec3::Y * 100.0, Vec3::ZERO, 7);
        let segs: usize = lines.iter().map(|(l, _)| l.len() - 1).sum();
        let mesh = bolt_mesh(&lines, Vec3::new(80.0, 10.0, 0.0), Vec3::ZERO);
        assert_eq!(mesh.count_vertices(), segs * 2 * 6);
    }
}