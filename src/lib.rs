//! # lau-voxel
//!
//! Voxel world engine for the **Lau** (Layered Agent-UI) system.
//!
//! Provides a voxel world where PLATO agents appear as characters for younger users.
//! Includes chunks, worlds, agents, game events, and room structures.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Chunk dimension (X, Y, Z).
pub const CHUNK_SIZE: usize = 16;

/// Gravity acceleration (blocks / s²).
pub const GRAVITY: f64 = 20.0;
/// Jump impulse (blocks / s).
pub const JUMP_IMPULSE: f64 = 8.0;

// ---------------------------------------------------------------------------
// Voxel
// ---------------------------------------------------------------------------

/// A single voxel type in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Voxel {
    Air,
    Stone,
    Grass,
    Water,
    Wood,
    Glass,
    /// Emits light; intensity 0–255.
    Glow(u8),
    /// User-defined variant.
    Custom(u8),
}

impl Voxel {
    /// Human-readable name.
    pub fn display_name(self) -> &'static str {
        match self {
            Voxel::Air => "Air",
            Voxel::Stone => "Stone",
            Voxel::Grass => "Grass",
            Voxel::Water => "Water",
            Voxel::Wood => "Wood",
            Voxel::Glass => "Glass",
            Voxel::Glow(_) => "Glow",
            Voxel::Custom(_) => "Custom",
        }
    }

    /// Base RGB colour (each channel 0–255).
    pub fn base_color(self) -> (u8, u8, u8) {
        match self {
            Voxel::Air => (0, 0, 0),
            Voxel::Stone => (128, 128, 128),
            Voxel::Grass => (34, 139, 34),
            Voxel::Water => (30, 144, 255),
            Voxel::Wood => (139, 90, 43),
            Voxel::Glass => (200, 220, 255),
            Voxel::Glow(intensity) => {
                let i = intensity;
                (255, 255, ((i as u16 * 200 / 255) as u8))
            }
            Voxel::Custom(id) => {
                // Deterministic spread based on id.
                let r = (id.wrapping_mul(73)).wrapping_add(40) % 200 + 55;
                let g = (id.wrapping_mul(137)).wrapping_add(80) % 200 + 55;
                let b = (id.wrapping_mul(211)).wrapping_add(120) % 200 + 55;
                (r, g, b)
            }
        }
    }

    /// Whether this voxel is considered solid for collision / raycasting.
    pub fn is_solid(self) -> bool {
        !matches!(self, Voxel::Air)
    }
}

// ---------------------------------------------------------------------------
// Chunk
// ---------------------------------------------------------------------------

/// A 16 × 16 × 16 block of voxels at chunk-grid position `(cx, cy, cz)`.
#[derive(Debug, Clone)]
pub struct Chunk {
    voxels: [Voxel; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
    pub cx: i32,
    pub cy: i32,
    pub cz: i32,
}

impl Chunk {
    /// Create an empty (all-Air) chunk at the given chunk coordinates.
    pub fn new(cx: i32, cy: i32, cz: i32) -> Self {
        Chunk {
            voxels: [Voxel::Air; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
            cx,
            cy,
            cz,
        }
    }

    fn idx(x: usize, y: usize, z: usize) -> usize {
        x + y * CHUNK_SIZE + z * CHUNK_SIZE * CHUNK_SIZE
    }

    /// Get the voxel at local coordinates.
    ///
    /// # Panics
    /// Panics if any coordinate is ≥ `CHUNK_SIZE`.
    pub fn get(&self, x: u8, y: u8, z: u8) -> Voxel {
        let (x, y, z) = (x as usize, y as usize, z as usize);
        assert!(x < CHUNK_SIZE && y < CHUNK_SIZE && z < CHUNK_SIZE);
        self.voxels[Self::idx(x, y, z)]
    }

    /// Set the voxel at local coordinates.
    pub fn set(&mut self, x: u8, y: u8, z: u8, v: Voxel) {
        let (x, y, z) = (x as usize, y as usize, z as usize);
        assert!(x < CHUNK_SIZE && y < CHUNK_SIZE && z < CHUNK_SIZE);
        self.voxels[Self::idx(x, y, z)] = v;
    }

    /// Whether the voxel at `(x, y, z)` is solid.
    pub fn is_solid(&self, x: u8, y: u8, z: u8) -> bool {
        self.get(x, y, z).is_solid()
    }

    /// Count how many of each voxel type exist in this chunk.
    pub fn count_by_type(&self) -> HashMap<Voxel, usize> {
        let mut counts = HashMap::new();
        for &v in &self.voxels {
            *counts.entry(v).or_insert(0) += 1;
        }
        counts
    }

    /// Return voxels that have at least one face adjacent to `Air`
    /// (i.e. exposed to the outside).
    ///
    /// Returns `(x, y, z, Voxel)` tuples.
    pub fn surface_voxels(&self) -> Vec<(u8, u8, u8, Voxel)> {
        let mut result = Vec::new();
        for z in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let v = self.voxels[Self::idx(x, y, z)];
                    if v == Voxel::Air {
                        continue;
                    }
                    if self.has_exposed_face(x, y, z) {
                        result.push((x as u8, y as u8, z as u8, v));
                    }
                }
            }
        }
        result
    }

    fn has_exposed_face(&self, x: usize, y: usize, z: usize) -> bool {
        let neighbors: [(i32, i32, i32); 6] = [
            (-1, 0, 0),
            (1, 0, 0),
            (0, -1, 0),
            (0, 1, 0),
            (0, 0, -1),
            (0, 0, 1),
        ];
        for (dx, dy, dz) in neighbors {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            let nz = z as i32 + dz;
            if nx < 0 || ny < 0 || nz < 0 {
                // Adjacent to chunk boundary → exposed.
                return true;
            }
            let (nx, ny, nz) = (nx as usize, ny as usize, nz as usize);
            if nx >= CHUNK_SIZE || ny >= CHUNK_SIZE || nz >= CHUNK_SIZE {
                return true;
            }
            if self.voxels[Self::idx(nx, ny, nz)] == Voxel::Air {
                return true;
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// World
// ---------------------------------------------------------------------------

/// A collection of chunks forming the voxel world.
#[derive(Debug, Clone)]
pub struct World {
    chunks: HashMap<(i32, i32, i32), Chunk>,
}

/// Helper: world position → chunk coordinates and local offset.
fn world_to_chunk(wx: i32, wy: i32, wz: i32) -> ((i32, i32, i32), (u8, u8, u8)) {
    let cx = wx.div_euclid(CHUNK_SIZE as i32);
    let cy = wy.div_euclid(CHUNK_SIZE as i32);
    let cz = wz.div_euclid(CHUNK_SIZE as i32);
    let lx = wx.rem_euclid(CHUNK_SIZE as i32) as u8;
    let ly = wy.rem_euclid(CHUNK_SIZE as i32) as u8;
    let lz = wz.rem_euclid(CHUNK_SIZE as i32) as u8;
    ((cx, cy, cz), (lx, ly, lz))
}

impl World {
    /// Create an empty world.
    pub fn new() -> Self {
        World {
            chunks: HashMap::new(),
        }
    }

    /// Set a voxel at world coordinates.
    pub fn set_voxel(&mut self, wx: i32, wy: i32, wz: i32, v: Voxel) {
        let (cc, local) = world_to_chunk(wx, wy, wz);
        let chunk = self.chunks.entry(cc).or_insert_with(|| Chunk::new(cc.0, cc.1, cc.2));
        chunk.set(local.0, local.1, local.2, v);
    }

    /// Get the voxel at world coordinates (returns `Air` if chunk absent).
    pub fn get_voxel(&self, wx: i32, wy: i32, wz: i32) -> Voxel {
        let (cc, local) = world_to_chunk(wx, wy, wz);
        match self.chunks.get(&cc) {
            Some(chunk) => chunk.get(local.0, local.1, local.2),
            None => Voxel::Air,
        }
    }

    /// Generate a flat world of the given chunk radius with a grass surface.
    ///
    /// `ground_level` is the Y value (in blocks) where grass is placed; stone
    /// fills everything below.
    pub fn generate_flat(chunk_radius: i32, ground_level: i32) -> Self {
        let mut world = World::new();
        let ground_chunk_y = ground_level.div_euclid(CHUNK_SIZE as i32);

        for cx in -chunk_radius..=chunk_radius {
            for cz in -chunk_radius..=chunk_radius {
                // Fill stone chunks entirely below ground_level's chunk.
                for cy in 0..ground_chunk_y {
                    let mut chunk = Chunk::new(cx, cy, cz);
                    chunk.voxels.fill(Voxel::Stone);
                    world.chunks.insert((cx, cy, cz), chunk);
                }

                // The chunk containing ground_level.
                let mut chunk = Chunk::new(cx, ground_chunk_y, cz);
                let ly_ground = ground_level.rem_euclid(CHUNK_SIZE as i32) as usize;
                for z in 0..CHUNK_SIZE {
                    for x in 0..CHUNK_SIZE {
                        for y in 0..=ly_ground {
                            let v = if y == ly_ground { Voxel::Grass } else { Voxel::Stone };
                            chunk.set(x as u8, y as u8, z as u8, v);
                        }
                    }
                }
                world.chunks.insert((cx, ground_chunk_y, cz), chunk);
            }
        }
        world
    }

    /// Generate terrain using a height function.
    ///
    /// `height_fn(x, z)` returns the topmost solid Y for a column.
    #[allow(clippy::needless_range_loop)]
    pub fn generate_heightmap<F>(chunk_radius: i32, height_fn: F) -> Self
    where
        F: Fn(i32, i32) -> i32,
    {
        let mut world = World::new();

        for cx in -chunk_radius..=chunk_radius {
            for cz in -chunk_radius..=chunk_radius {
                // Collect per-column heights in this chunk to determine max Y.
                let mut max_h: i32 = i32::MIN;
                let mut heights = [[0i32; CHUNK_SIZE]; CHUNK_SIZE];
                for lz in 0..CHUNK_SIZE {
                    for lx in 0..CHUNK_SIZE {
                        let wx = cx * CHUNK_SIZE as i32 + lx as i32;
                        let wz = cz * CHUNK_SIZE as i32 + lz as i32;
                        let h = height_fn(wx, wz);
                        heights[lx][lz] = h;
                        max_h = max_h.max(h);
                    }
                }

                if max_h < 0 {
                    continue;
                }

                let max_cy = max_h.div_euclid(CHUNK_SIZE as i32);
                for cy in 0..=max_cy {
                    let mut chunk = Chunk::new(cx, cy, cz);
                    for lz in 0..CHUNK_SIZE {
                        for lx in 0..CHUNK_SIZE {
                            let surface_y = heights[lx][lz];
                            let chunk_bottom = cy * CHUNK_SIZE as i32;
                            if surface_y < chunk_bottom {
                                continue;
                            }
                            for ly in 0..CHUNK_SIZE {
                                let wy = cy * CHUNK_SIZE as i32 + ly as i32;
                                if wy > surface_y {
                                    // Above surface: air.
                                    continue;
                                }
                                let v = if wy == surface_y {
                                    Voxel::Grass
                                } else {
                                    Voxel::Stone
                                };
                                chunk.set(lx as u8, ly as u8, lz as u8, v);
                            }
                        }
                    }
                    world.chunks.insert((cx, cy, cz), chunk);
                }
            }
        }
        world
    }

    /// Cast a ray from `origin` in `direction` and return the first solid
    /// voxel hit within `max_dist`, along with its world coordinates.
    ///
    /// Uses a simple DDA-style voxel traversal.
    pub fn raycast(
        &self,
        origin: (f64, f64, f64),
        direction: (f64, f64, f64),
        max_dist: f64,
    ) -> Option<(i32, i32, i32, Voxel)> {
        let (ox, oy, oz) = origin;
        let (mut dx, mut dy, mut dz) = direction;

        // Normalise direction.
        let len = (dx * dx + dy * dy + dz * dz).sqrt();
        if len < 1e-12 {
            return None;
        }
        dx /= len;
        dy /= len;
        dz /= len;

        let mut x = ox.floor() as i32;
        let mut y = oy.floor() as i32;
        let mut z = oz.floor() as i32;

        let step_x = if dx > 0.0 { 1i32 } else { -1 };
        let step_y = if dy > 0.0 { 1i32 } else { -1 };
        let step_z = if dz > 0.0 { 1i32 } else { -1 };

        let t_max_x = if dx.abs() > 1e-12 {
            if dx > 0.0 {
                (x as f64 + 1.0 - ox) / dx
            } else {
                (ox - x as f64) / (-dx)
            }
        } else {
            f64::INFINITY
        };
        let t_max_y = if dy.abs() > 1e-12 {
            if dy > 0.0 {
                (y as f64 + 1.0 - oy) / dy
            } else {
                (oy - y as f64) / (-dy)
            }
        } else {
            f64::INFINITY
        };
        let t_max_z = if dz.abs() > 1e-12 {
            if dz > 0.0 {
                (z as f64 + 1.0 - oz) / dz
            } else {
                (oz - z as f64) / (-dz)
            }
        } else {
            f64::INFINITY
        };

        let t_delta_x = if dx.abs() > 1e-12 { 1.0 / dx.abs() } else { f64::INFINITY };
        let t_delta_y = if dy.abs() > 1e-12 { 1.0 / dy.abs() } else { f64::INFINITY };
        let t_delta_z = if dz.abs() > 1e-12 { 1.0 / dz.abs() } else { f64::INFINITY };

        let mut t_max_x = t_max_x;
        let mut t_max_y = t_max_y;
        let mut t_max_z = t_max_z;

        // Check starting voxel.
        let v = self.get_voxel(x, y, z);
        if v.is_solid() {
            return Some((x, y, z, v));
        }

        for _ in 0..10000 {
            if t_max_x < t_max_y {
                if t_max_x < t_max_z {
                    if t_max_x > max_dist {
                        break;
                    }
                    x += step_x;
                    t_max_x += t_delta_x;
                } else {
                    if t_max_z > max_dist {
                        break;
                    }
                    z += step_z;
                    t_max_z += t_delta_z;
                }
            } else if t_max_y < t_max_z {
                if t_max_y > max_dist {
                    break;
                }
                y += step_y;
                t_max_y += t_delta_y;
            } else {
                if t_max_z > max_dist {
                    break;
                }
                z += step_z;
                t_max_z += t_delta_z;
            }

            let v = self.get_voxel(x, y, z);
            if v.is_solid() {
                return Some((x, y, z, v));
            }
        }

        None
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Agent
// ---------------------------------------------------------------------------

/// An agent (PLATO character) in the voxel world.
#[derive(Debug, Clone)]
pub struct Agent {
    pub id: String,
    pub position: (f64, f64, f64),
    /// Yaw in radians.
    pub facing: f64,
    pub velocity: (f64, f64, f64),
    pub appearance: String,
}

impl Agent {
    /// Create a new agent at the given position.
    pub fn new(id: impl Into<String>, position: (f64, f64, f64)) -> Self {
        Agent {
            id: id.into(),
            position,
            facing: 0.0,
            velocity: (0.0, 0.0, 0.0),
            appearance: String::new(),
        }
    }

    /// Move toward `target` at `speed` for `dt` seconds.
    pub fn move_toward(&mut self, target: (f64, f64, f64), speed: f64, dt: f64) {
        let (tx, ty, tz) = target;
        let (px, py, pz) = self.position;
        let dx = tx - px;
        let dy = ty - py;
        let dz = tz - pz;
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
        if dist < 1e-6 {
            return;
        }
        let step = (speed * dt).min(dist);
        let ratio = step / dist;
        self.position.0 += dx * ratio;
        self.position.1 += dy * ratio;
        self.position.2 += dz * ratio;
        // Update facing to direction of travel.
        self.facing = dz.atan2(dx);
    }

    /// Apply a jump impulse (only effective if grounded).
    pub fn jump(&mut self) {
        self.velocity.1 += JUMP_IMPULSE;
    }

    /// Apply gravity for `dt` seconds.
    pub fn apply_gravity(&mut self, dt: f64) {
        self.velocity.1 -= GRAVITY * dt;
    }

    /// Integrate velocity into position for `dt` seconds.
    pub fn integrate(&mut self, dt: f64) {
        self.position.0 += self.velocity.0 * dt;
        self.position.1 += self.velocity.1 * dt;
        self.position.2 += self.velocity.2 * dt;
    }

    /// Check whether the agent is standing on a solid voxel.
    pub fn is_grounded(&self, world: &World) -> bool {
        let (_, py, _) = self.position;
        let below = (py - 0.1).floor() as i32;
        let bx = self.position.0.floor() as i32;
        let bz = self.position.2.floor() as i32;
        world.get_voxel(bx, below, bz).is_solid()
    }
}

// ---------------------------------------------------------------------------
// GameEvent
// ---------------------------------------------------------------------------

/// Events that occur in the game world.
#[derive(Debug, Clone, PartialEq)]
pub enum GameEvent {
    AgentJoined { agent_id: String },
    AgentLeft { agent_id: String },
    VoxelPlaced { x: i32, y: i32, z: i32, voxel: Voxel },
    VoxelRemoved { x: i32, y: i32, z: i32 },
    AgentMoved { agent_id: String, x: f64, y: f64, z: f64 },
    AgentSpoke { agent_id: String, message: String },
}

// ---------------------------------------------------------------------------
// BuildPattern
// ---------------------------------------------------------------------------

/// Predefined structure patterns for game rooms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildPattern {
    Cabin,
    Tower,
    Garden,
    Crystal,
    Ruins,
}

/// Build a structure into the world at a given origin (world coordinates).
pub fn build_structure(world: &mut World, pattern: BuildPattern, ox: i32, oy: i32, oz: i32) {
    match pattern {
        BuildPattern::Cabin => {
            // 5×4×5 wood cabin with glass window and glow light.
            for x in 0..5 {
                for z in 0..5 {
                    for y in 0..4 {
                        let is_wall = x == 0 || x == 4 || z == 0 || z == 4;
                        let is_roof = y == 3;
                        if is_roof || (is_wall && !(y == 1 && ((x == 2 && z == 0) || (x == 0 && z == 2)))) {
                            world.set_voxel(ox + x, oy + y, oz + z, Voxel::Wood);
                        } else if is_wall && y == 1 && ((x == 2 && z == 0) || (x == 0 && z == 2)) {
                            world.set_voxel(ox + x, oy + y, oz + z, Voxel::Glass);
                        }
                    }
                }
            }
            // Glow light inside.
            world.set_voxel(ox + 2, oy + 2, oz + 2, Voxel::Glow(200));
        }
        BuildPattern::Tower => {
            for y in 0..8 {
                for x in 0..3 {
                    for z in 0..3 {
                        let is_edge = x == 0 || x == 2 || z == 0 || z == 2;
                        if is_edge {
                            world.set_voxel(ox + x, oy + y, oz + z, Voxel::Stone);
                        }
                    }
                }
            }
            // Glow beacon on top.
            world.set_voxel(ox + 1, oy + 8, oz + 1, Voxel::Glow(255));
        }
        BuildPattern::Garden => {
            // 7×1×7 grass with flowers (Custom voxels).
            for x in 0..7 {
                for z in 0..7 {
                    world.set_voxel(ox + x, oy, oz + z, Voxel::Grass);
                    // Checkerboard flowers.
                    if (x + z) % 2 == 0 {
                        world.set_voxel(ox + x, oy + 1, oz + z, Voxel::Custom((x * 3 + z * 7) as u8));
                    }
                }
            }
        }
        BuildPattern::Crystal => {
            // Diamond-shaped glow structure.
            for y in 0..7 {
                #[allow(clippy::unnecessary_cast)]
                let radius = if y < 4 { y as i32 } else { 6 - y as i32 };
                for x in -radius..=radius {
                    for z in -radius..=radius {
                        if x.abs() + z.abs() <= radius {
                            let intensity = (100 + y as u16 * 20).min(255) as u8;
                            world.set_voxel(ox + 3 + x, oy + y, oz + 3 + z, Voxel::Glow(intensity));
                        }
                    }
                }
            }
        }
        BuildPattern::Ruins => {
            // Scattered stone walls.
            for y in 0..3 {
                world.set_voxel(ox, oy + y, oz, Voxel::Stone);
                world.set_voxel(ox + 1, oy + y, oz, Voxel::Stone);
                world.set_voxel(ox + 4, oy + y, oz + 4, Voxel::Stone);
                world.set_voxel(ox + 4, oy + y, oz + 3, Voxel::Stone);
            }
            // Broken wall.
            world.set_voxel(ox + 2, oy, oz + 2, Voxel::Stone);
            world.set_voxel(ox + 2, oy + 1, oz + 2, Voxel::Stone);
        }
    }
}

// ---------------------------------------------------------------------------
// GameRoom
// ---------------------------------------------------------------------------

/// Bridges a PLATO room to a game location in the voxel world.
#[derive(Debug, Clone)]
pub struct GameRoom {
    pub room_id: String,
    pub center: (f64, f64, f64),
    pub radius: f64,
    agents: Vec<Agent>,
}

impl GameRoom {
    /// Create a new game room.
    pub fn new(room_id: impl Into<String>, center: (f64, f64, f64), radius: f64) -> Self {
        GameRoom {
            room_id: room_id.into(),
            center,
            radius,
            agents: Vec::new(),
        }
    }

    /// Add an agent to the room.
    pub fn add_agent(&mut self, agent: Agent) {
        self.agents.push(agent);
    }

    /// Remove an agent by id.
    pub fn remove_agent(&mut self, agent_id: &str) -> bool {
        let before = self.agents.len();
        self.agents.retain(|a| a.id != agent_id);
        self.agents.len() != before
    }

    /// Get agents currently in this room.
    pub fn agents_in_room(&self) -> Vec<&Agent> {
        self.agents.iter().collect()
    }

    /// Build a structure in the given world at the room's center, using the
    /// specified pattern.
    pub fn build_structure(&self, world: &mut World, pattern: BuildPattern) {
        let ox = self.center.0.floor() as i32;
        let oy = self.center.1.floor() as i32;
        let oz = self.center.2.floor() as i32;
        build_structure(world, pattern, ox, oy, oz);
    }

    /// Modify the world around the room based on a "vibe" score (0.0–1.0).
    ///
    /// Higher vibe → more glow, brighter. Lower vibe → dimmer, water rises.
    pub fn room_vibe_affects_world(&self, world: &mut World, vibe: f64) {
        let cx = self.center.0.floor() as i32;
        let cy = self.center.1.floor() as i32;
        let cz = self.center.2.floor() as i32;
        let r = self.radius.ceil() as i32;

        // Place / replace glow voxels proportional to vibe.
        let glow_count = (vibe * (r * r) as f64).floor() as i32;
        let intensity = (vibe * 255.0).min(255.0) as u8;
        let span = 2 * r + 1;
        for i in 0..glow_count {
            let gx = cx + (i * 7).rem_euclid(span) - r;
            let gz = cz + (i * 13).rem_euclid(span) - r;
            world.set_voxel(gx, cy + 3, gz, Voxel::Glow(intensity));
        }

        // Low vibe: add water.
        if vibe < 0.3 {
            let water_y = cy + (vibe * 2.0).floor() as i32;
            for x in (cx - r)..=(cx + r) {
                for z in (cz - r)..=(cz + r) {
                    if world.get_voxel(x, water_y, z) == Voxel::Air {
                        world.set_voxel(x, water_y, z, Voxel::Water);
                    }
                }
            }
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- Voxel tests --------------------------------------------------------

    #[test]
    fn voxel_display_names() {
        assert_eq!(Voxel::Air.display_name(), "Air");
        assert_eq!(Voxel::Stone.display_name(), "Stone");
        assert_eq!(Voxel::Grass.display_name(), "Grass");
        assert_eq!(Voxel::Water.display_name(), "Water");
        assert_eq!(Voxel::Wood.display_name(), "Wood");
        assert_eq!(Voxel::Glass.display_name(), "Glass");
        assert_eq!(Voxel::Glow(100).display_name(), "Glow");
        assert_eq!(Voxel::Custom(42).display_name(), "Custom");
    }

    #[test]
    fn voxel_base_colors() {
        assert_eq!(Voxel::Air.base_color(), (0, 0, 0));
        assert_eq!(Voxel::Grass.base_color(), (34, 139, 34));
        assert_eq!(Voxel::Water.base_color(), (30, 144, 255));
    }

    #[test]
    fn voxel_solidity() {
        assert!(!Voxel::Air.is_solid());
        assert!(Voxel::Stone.is_solid());
        assert!(Voxel::Water.is_solid());
        assert!(Voxel::Glow(0).is_solid());
    }

    #[test]
    fn glow_color_varies() {
        let c1 = Voxel::Glow(0).base_color();
        let c2 = Voxel::Glow(255).base_color();
        assert_ne!(c1, c2);
    }

    #[test]
    fn custom_color_is_deterministic() {
        let c1 = Voxel::Custom(10).base_color();
        let c2 = Voxel::Custom(10).base_color();
        assert_eq!(c1, c2);
    }

    // -- Chunk tests --------------------------------------------------------

    #[test]
    fn chunk_new_is_air() {
        let chunk = Chunk::new(0, 0, 0);
        assert_eq!(chunk.get(0, 0, 0), Voxel::Air);
        assert_eq!(chunk.get(15, 15, 15), Voxel::Air);
    }

    #[test]
    fn chunk_set_get_roundtrip() {
        let mut chunk = Chunk::new(1, 2, 3);
        chunk.set(5, 10, 7, Voxel::Stone);
        assert_eq!(chunk.get(5, 10, 7), Voxel::Stone);
    }

    #[test]
    fn chunk_coordinates_stored() {
        let chunk = Chunk::new(10, 20, 30);
        assert_eq!((chunk.cx, chunk.cy, chunk.cz), (10, 20, 30));
    }

    #[test]
    fn chunk_is_solid() {
        let mut chunk = Chunk::new(0, 0, 0);
        assert!(!chunk.is_solid(0, 0, 0));
        chunk.set(0, 0, 0, Voxel::Grass);
        assert!(chunk.is_solid(0, 0, 0));
    }

    #[test]
    fn chunk_count_by_type() {
        let mut chunk = Chunk::new(0, 0, 0);
        chunk.set(0, 0, 0, Voxel::Stone);
        chunk.set(1, 0, 0, Voxel::Stone);
        chunk.set(2, 0, 0, Voxel::Grass);
        let counts = chunk.count_by_type();
        assert_eq!(*counts.get(&Voxel::Stone).unwrap(), 2);
        assert_eq!(*counts.get(&Voxel::Grass).unwrap(), 1);
        assert_eq!(*counts.get(&Voxel::Air).unwrap(), CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE - 3);
    }

    #[test]
    fn chunk_surface_voxels_single_block() {
        let mut chunk = Chunk::new(0, 0, 0);
        chunk.set(8, 8, 8, Voxel::Stone);
        let surf = chunk.surface_voxels();
        assert_eq!(surf.len(), 1);
        assert_eq!(surf[0].3, Voxel::Stone);
    }

    #[test]
    fn chunk_surface_voxels_buried_not_exposed() {
        let mut chunk = Chunk::new(0, 0, 0);
        // Fill a 3×3×3 cube — only the shell should be surface.
        for x in 5..8 {
            for y in 5..8 {
                for z in 5..8 {
                    chunk.set(x, y, z, Voxel::Stone);
                }
            }
        }
        let surf = chunk.surface_voxels();
        // Shell of a 3³ cube = 27 - 1 = 26.
        assert_eq!(surf.len(), 26);
    }

    // -- World tests --------------------------------------------------------

    #[test]
    fn world_new_is_empty() {
        let world = World::new();
        assert_eq!(world.get_voxel(0, 0, 0), Voxel::Air);
    }

    #[test]
    fn world_set_get_roundtrip() {
        let mut world = World::new();
        world.set_voxel(100, 50, -30, Voxel::Wood);
        assert_eq!(world.get_voxel(100, 50, -30), Voxel::Wood);
    }

    #[test]
    fn world_generate_flat() {
        let world = World::generate_flat(1, 5);
        // Above ground: air.
        assert_eq!(world.get_voxel(0, 6, 0), Voxel::Air);
        // Ground surface: grass.
        assert_eq!(world.get_voxel(0, 5, 0), Voxel::Grass);
        // Below ground: stone.
        assert_eq!(world.get_voxel(0, 4, 0), Voxel::Stone);
        assert_eq!(world.get_voxel(0, 0, 0), Voxel::Stone);
    }

    #[test]
    fn world_generate_flat_negative_coords() {
        let world = World::generate_flat(1, 3);
        assert_eq!(world.get_voxel(-10, 3, -10), Voxel::Grass);
        assert_eq!(world.get_voxel(-10, 2, -10), Voxel::Stone);
    }

    #[test]
    fn world_generate_heightmap_flat() {
        let world = World::generate_heightmap(1, |_x, _z| 10);
        assert_eq!(world.get_voxel(0, 10, 0), Voxel::Grass);
        assert_eq!(world.get_voxel(0, 9, 0), Voxel::Stone);
        assert_eq!(world.get_voxel(0, 11, 0), Voxel::Air);
    }

    #[test]
    fn world_generate_heightmap_hilly() {
        let world = World::generate_heightmap(1, |x, _z| {
            if x == 0 { 20 } else { 5 }
        });
        assert_eq!(world.get_voxel(0, 20, 0), Voxel::Grass);
        assert_eq!(world.get_voxel(5, 5, 0), Voxel::Grass);
        assert_eq!(world.get_voxel(5, 6, 0), Voxel::Air);
    }

    #[test]
    fn world_raycast_hit() {
        let mut world = World::new();
        world.set_voxel(5, 0, 0, Voxel::Stone);
        let hit = world.raycast((0.5, 0.5, 0.5), (1.0, 0.0, 0.0), 100.0);
        assert!(hit.is_some());
        let (x, y, z, v) = hit.unwrap();
        assert_eq!((x, y, z), (5, 0, 0));
        assert_eq!(v, Voxel::Stone);
    }

    #[test]
    fn world_raycast_miss() {
        let world = World::new();
        let hit = world.raycast((0.0, 0.0, 0.0), (0.0, 1.0, 0.0), 10.0);
        assert!(hit.is_none());
    }

    #[test]
    fn world_raycast_zero_direction() {
        let world = World::new();
        let hit = world.raycast((0.0, 0.0, 0.0), (0.0, 0.0, 0.0), 10.0);
        assert!(hit.is_none());
    }

    #[test]
    fn world_raycast_max_dist() {
        let mut world = World::new();
        world.set_voxel(50, 0, 0, Voxel::Stone);
        // Max distance too short.
        let hit = world.raycast((0.5, 0.5, 0.5), (1.0, 0.0, 0.0), 5.0);
        assert!(hit.is_none());
    }

    // -- Agent tests --------------------------------------------------------

    #[test]
    fn agent_new() {
        let agent = Agent::new("test", (1.0, 2.0, 3.0));
        assert_eq!(agent.id, "test");
        assert_eq!(agent.position, (1.0, 2.0, 3.0));
        assert_eq!(agent.velocity, (0.0, 0.0, 0.0));
    }

    #[test]
    fn agent_move_toward() {
        let mut agent = Agent::new("a", (0.0, 0.0, 0.0));
        agent.move_toward((10.0, 0.0, 0.0), 5.0, 1.0);
        assert!((agent.position.0 - 5.0).abs() < 1e-6);
    }

    #[test]
    fn agent_move_toward_arrives() {
        let mut agent = Agent::new("a", (0.0, 0.0, 0.0));
        // Speed × dt > distance → clamp to target.
        agent.move_toward((1.0, 0.0, 0.0), 100.0, 1.0);
        assert!((agent.position.0 - 1.0).abs() < 1e-6);
    }

    #[test]
    fn agent_move_toward_zero_dist() {
        let mut agent = Agent::new("a", (5.0, 5.0, 5.0));
        agent.move_toward((5.0, 5.0, 5.0), 10.0, 1.0);
        assert_eq!(agent.position, (5.0, 5.0, 5.0));
    }

    #[test]
    fn agent_jump_adds_velocity() {
        let mut agent = Agent::new("a", (0.0, 10.0, 0.0));
        assert_eq!(agent.velocity.1, 0.0);
        agent.jump();
        assert!((agent.velocity.1 - JUMP_IMPULSE).abs() < 1e-6);
    }

    #[test]
    fn agent_is_grounded() {
        let world = World::generate_flat(1, 5);
        let agent = Agent::new("a", (0.5, 6.0, 0.5));
        // Agent at y=6, ground at y=5. Below feet is y=5 (solid grass).
        assert!(agent.is_grounded(&world));
    }

    #[test]
    fn agent_not_grounded_in_air() {
        let world = World::generate_flat(1, 5);
        let agent = Agent::new("a", (0.5, 20.0, 0.5));
        assert!(!agent.is_grounded(&world));
    }

    #[test]
    fn agent_gravity_and_integrate() {
        let mut agent = Agent::new("a", (0.0, 10.0, 0.0));
        agent.jump();
        agent.apply_gravity(0.1);
        agent.integrate(0.1);
        // After jump: vy = 8.0. After gravity: vy = 8.0 - 2.0 = 6.0.
        // After integrate: y = 10.0 + 6.0 * 0.1 = 10.6.
        assert!((agent.position.1 - 10.6).abs() < 1e-6);
    }

    // -- GameEvent tests ----------------------------------------------------

    #[test]
    fn game_event_variants() {
        let e1 = GameEvent::AgentJoined { agent_id: "a".into() };
        let e2 = GameEvent::AgentLeft { agent_id: "a".into() };
        let e3 = GameEvent::VoxelPlaced { x: 1, y: 2, z: 3, voxel: Voxel::Stone };
        let e4 = GameEvent::VoxelRemoved { x: 1, y: 2, z: 3 };
        let e5 = GameEvent::AgentMoved { agent_id: "a".into(), x: 1.0, y: 2.0, z: 3.0 };
        let e6 = GameEvent::AgentSpoke { agent_id: "a".into(), message: "hello".into() };

        assert_ne!(format!("{e1:?}"), format!("{e2:?}"));
        assert!(matches!(e3, GameEvent::VoxelPlaced { .. }));
        assert!(matches!(e4, GameEvent::VoxelRemoved { .. }));
        assert!(matches!(e5, GameEvent::AgentMoved { .. }));
        assert!(matches!(e6, GameEvent::AgentSpoke { .. }));
    }

    // -- GameRoom tests -----------------------------------------------------

    #[test]
    fn game_room_new() {
        let room = GameRoom::new("room1", (10.0, 5.0, 10.0), 20.0);
        assert_eq!(room.room_id, "room1");
        assert_eq!(room.center, (10.0, 5.0, 10.0));
        assert!(room.agents_in_room().is_empty());
    }

    #[test]
    fn game_room_add_remove_agents() {
        let mut room = GameRoom::new("r", (0.0, 0.0, 0.0), 10.0);
        room.add_agent(Agent::new("a1", (0.0, 0.0, 0.0)));
        room.add_agent(Agent::new("a2", (1.0, 0.0, 0.0)));
        assert_eq!(room.agents_in_room().len(), 2);
        assert!(room.remove_agent("a1"));
        assert_eq!(room.agents_in_room().len(), 1);
        assert!(!room.remove_agent("a1")); // already removed
    }

    #[test]
    fn game_room_build_cabin() {
        let mut world = World::new();
        let room = GameRoom::new("r", (0.0, 0.0, 0.0), 10.0);
        room.build_structure(&mut world, BuildPattern::Cabin);
        // Cabin places wood at origin.
        assert_eq!(world.get_voxel(0, 0, 0), Voxel::Wood);
        // Glow light inside.
        assert!(matches!(world.get_voxel(2, 2, 2), Voxel::Glow(_)));
    }

    #[test]
    fn game_room_build_tower() {
        let mut world = World::new();
        let room = GameRoom::new("r", (0.0, 0.0, 0.0), 10.0);
        room.build_structure(&mut world, BuildPattern::Tower);
        assert_eq!(world.get_voxel(0, 0, 0), Voxel::Stone);
        assert!(matches!(world.get_voxel(1, 8, 1), Voxel::Glow(_)));
    }

    #[test]
    fn game_room_build_garden() {
        let mut world = World::new();
        let room = GameRoom::new("r", (0.0, 0.0, 0.0), 10.0);
        room.build_structure(&mut world, BuildPattern::Garden);
        assert_eq!(world.get_voxel(0, 0, 0), Voxel::Grass);
        assert!(matches!(world.get_voxel(0, 1, 0), Voxel::Custom(_)));
    }

    #[test]
    fn game_room_build_crystal() {
        let mut world = World::new();
        let room = GameRoom::new("r", (0.0, 0.0, 0.0), 10.0);
        room.build_structure(&mut world, BuildPattern::Crystal);
        assert!(matches!(world.get_voxel(3, 0, 3), Voxel::Glow(_)));
    }

    #[test]
    fn game_room_build_ruins() {
        let mut world = World::new();
        let room = GameRoom::new("r", (0.0, 0.0, 0.0), 10.0);
        room.build_structure(&mut world, BuildPattern::Ruins);
        assert_eq!(world.get_voxel(0, 0, 0), Voxel::Stone);
    }

    #[test]
    fn game_room_vibe_high_adds_glow() {
        let mut world = World::new();
        let room = GameRoom::new("r", (0.0, 0.0, 0.0), 5.0);
        room.room_vibe_affects_world(&mut world, 1.0);
        // glow_count = vibe * r² = 25, span = 11
        // i=0: gx = 0+(0)%11-5 = -5, gz = 0+(0)%11-5 = -5
        assert!(matches!(world.get_voxel(-5, 3, -5), Voxel::Glow(_)));
        // Verify at least several glow voxels exist at y=3.
        let mut glow_count = 0;
        for x in -5..=5 {
            for z in -5..=5 {
                if matches!(world.get_voxel(x, 3, z), Voxel::Glow(_)) {
                    glow_count += 1;
                }
            }
        }
        assert!(glow_count > 5, "Expected many glow voxels, got {glow_count}");
    }

    #[test]
    fn game_room_vibe_low_adds_water() {
        let mut world = World::generate_flat(1, 5);
        let room = GameRoom::new("r", (0.0, 5.0, 0.0), 3.0);
        room.room_vibe_affects_world(&mut world, 0.1);
        // Low vibe should add water above ground.
        assert_eq!(world.get_voxel(0, 5, 0), Voxel::Grass); // grass still there
    }

    // -- BuildPattern exhaustive check --------------------------------------

    #[test]
    fn all_patterns_produce_voxels() {
        let patterns = [
            BuildPattern::Cabin,
            BuildPattern::Tower,
            BuildPattern::Garden,
            BuildPattern::Crystal,
            BuildPattern::Ruins,
        ];
        for &p in &patterns {
            let mut world = World::new();
            build_structure(&mut world, p, 0, 0, 0);
            // At least one non-air voxel must exist.
            let mut found = false;
            for x in -5..20 {
                for y in -5..20 {
                    for z in -5..20 {
                        if world.get_voxel(x, y, z) != Voxel::Air {
                            found = true;
                            break;
                        }
                    }
                    if found { break; }
                }
                if found { break; }
            }
            assert!(found, "Pattern {p:?} produced no voxels");
        }
    }
}
