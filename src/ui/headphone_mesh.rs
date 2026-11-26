use std::f32::consts::PI;

pub const STRIDE_FLOATS: usize = 7;

pub const MAT_SHELL: f32 = 0.0;
pub const MAT_CUSHION: f32 = 1.0;
pub const MAT_METAL: f32 = 2.0;
pub const MAT_SCRIM: f32 = 3.0;

pub struct MeshData {
    pub vertices: Vec<f32>,
    pub indices: Vec<u32>,
}

#[derive(Clone, Copy)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Vec3 {
    const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    fn normalized(self) -> Self {
        let l = self.length();
        if l > 1e-6 {
            Self::new(self.x / l, self.y / l, self.z / l)
        } else {
            Self::new(0.0, 1.0, 0.0)
        }
    }

    fn cross(self, o: Self) -> Self {
        Self::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
}

impl std::ops::Add for Vec3 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl std::ops::Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }
}

struct MeshBuffer {
    verts: Vec<f32>,
    tris: Vec<u32>,
}

impl MeshBuffer {
    fn new() -> Self {
        Self {
            verts: Vec::with_capacity(4096 * STRIDE_FLOATS),
            tris: Vec::with_capacity(16384),
        }
    }

    fn add_vertex(&mut self, p: Vec3, n: Vec3, mat_id: f32) -> u32 {
        let idx = (self.verts.len() / STRIDE_FLOATS) as u32;
        let nn = n.normalized();
        self.verts.extend_from_slice(&[p.x, p.y, p.z, nn.x, nn.y, nn.z, mat_id]);
        idx
    }

    fn add_tri(&mut self, a: u32, b: u32, c: u32) {
        self.tris.push(a);
        self.tris.push(b);
        self.tris.push(c);
    }

    fn add_quad(&mut self, i00: u32, i10: u32, i11: u32, i01: u32) {
        self.add_tri(i00, i10, i11);
        self.add_tri(i00, i11, i01);
    }

    fn append(&mut self, src: &MeshBuffer) {
        let base = (self.verts.len() / STRIDE_FLOATS) as u32;
        self.verts.extend_from_slice(&src.verts);
        for &idx in &src.tris {
            self.tris.push(base + idx);
        }
    }

    fn mirror_part_x(&mut self, src: &MeshBuffer) {
        let base = (self.verts.len() / STRIDE_FLOATS) as u32;
        let v_count = src.verts.len() / STRIDE_FLOATS;
        for v in 0..v_count {
            let off = v * STRIDE_FLOATS;
            self.verts.push(-src.verts[off]);
            self.verts.push(src.verts[off + 1]);
            self.verts.push(src.verts[off + 2]);
            self.verts.push(-src.verts[off + 3]);
            self.verts.push(src.verts[off + 4]);
            self.verts.push(src.verts[off + 5]);
            self.verts.push(src.verts[off + 6]);
        }
        let mut t = 0;
        while t < src.tris.len() {
            self.tris.push(base + src.tris[t]);
            self.tris.push(base + src.tris[t + 2]);
            self.tris.push(base + src.tris[t + 1]);
            t += 3;
        }
    }

    fn translate_y(&mut self, dy: f32) {
        let mut i = 1;
        while i < self.verts.len() {
            self.verts[i] += dy;
            i += STRIDE_FLOATS;
        }
    }

    fn into_mesh_data(self) -> MeshData {
        MeshData {
            vertices: self.verts,
            indices: self.tris,
        }
    }
}

pub fn build_xm5_mesh() -> MeshData {
    let mut root = MeshBuffer::new();
    add_headband_arch(&mut root);
    add_headband_crown_cushion(&mut root);

    let mut right_side = MeshBuffer::new();
    build_right_side_assembly(&mut right_side);

    root.append(&right_side);
    root.mirror_part_x(&right_side);
    root.translate_y(-0.04);
    root.into_mesh_data()
}

pub fn build_earbuds_mesh() -> MeshData {
    let mut root = MeshBuffer::new();
    let mut right_bud = MeshBuffer::new();
    build_right_earbud_assembly(&mut right_bud);

    root.append(&right_bud);
    root.mirror_part_x(&right_bud);
    root.translate_y(-0.04);
    root.into_mesh_data()
}

fn build_right_earbud_assembly(side: &mut MeshBuffer) {
    let bud_center = Vec3::new(0.49, 0.04, 0.01);
    let bud_out = Vec3::new(0.76, 0.16, 0.63).normalized();
    let ref_up = Vec3::new(-0.12, 0.98, 0.14).normalized();
    let bud_fwd = bud_out.cross(ref_up).normalized();
    let bud_up = bud_fwd.cross(bud_out).normalized();

    let r_up = 0.295;
    let r_fwd = 0.235;
    let outer_depth = 0.145;
    let inner_depth = 0.165;

    let shell_surface = |s: f32, beta: f32| -> Vec3 {
        let ss = s.sin();
        let cc = s.cos();
        let w = 0.5 + 0.5 * cc;
        let depth = inner_depth + (outer_depth - inner_depth) * w;
        let axial = depth * cc * (1.0 + 0.32 * ss * ss);
        let ergo = 1.0
            + 0.085 * ss * (beta - 0.35).sin()
            + 0.045 * ss * ss * (2.0 * beta + 0.3).cos();
        let rad = ss * ergo;
        bud_center
            + bud_up * (r_up * rad * beta.cos())
            + bud_fwd * (r_fwd * rad * beta.sin())
            + bud_out * axial
    };

    let shell_normal = |s: f32, beta: f32| -> Vec3 {
        let eps = 0.002;
        let s0 = (s - eps).max(0.0005);
        let s1 = (s + eps).min(PI - 0.0005);
        let t_lat = shell_surface(s1, beta) - shell_surface(s0, beta);
        let t_lon = shell_surface(s, beta + eps) - shell_surface(s, beta - eps);
        t_lat.cross(t_lon).normalized()
    };

    add_earbud_shell(
        side,
        bud_center,
        bud_out,
        outer_depth,
        inner_depth,
        &shell_surface,
        &shell_normal,
    );
    add_touch_accent_ring(side, &shell_surface, &shell_normal);
    add_earbud_mic_port(side, shell_surface(1.15, 0.65), shell_normal(1.15, 0.65));

    let nozzle_dir = (bud_out * -0.68 + bud_fwd * 0.68 - bud_up * 0.26).normalized();
    let nozzle_start = bud_center - bud_out * 0.055 + bud_fwd * 0.055 - bud_up * 0.025;
    let nozzle_tip = nozzle_start + nozzle_dir * 0.185;

    add_swept_bezier_tube(
        side,
        nozzle_start,
        nozzle_start + nozzle_dir * 0.06,
        nozzle_tip - nozzle_dir * 0.06,
        nozzle_tip,
        0.074,
        0.058,
        8,
        24,
        MAT_SHELL,
    );

    let tip_base = nozzle_tip - nozzle_dir * 0.042;
    add_silicone_ear_tip(side, tip_base, nozzle_dir);
    add_nozzle_acoustic_scrim(side, tip_base, nozzle_dir);
}

fn add_earbud_shell(
    out: &mut MeshBuffer,
    center: Vec3,
    bud_out: Vec3,
    outer_depth: f32,
    inner_depth: f32,
    surface_at: &dyn Fn(f32, f32) -> Vec3,
    normal_at: &dyn Fn(f32, f32) -> Vec3,
) {
    let lat_segs = 28usize;
    let lon_segs = 40usize;

    let outer_apex = out.add_vertex(center + bud_out * outer_depth, bud_out, MAT_SHELL);
    let inner_apex = out.add_vertex(center - bud_out * inner_depth, bud_out * -1.0, MAT_SHELL);

    let rings: Vec<Vec<u32>> = (0..lat_segs)
        .map(|i| {
            let s = ((i + 1) as f32 / (lat_segs + 1) as f32) * PI;
            (0..lon_segs)
                .map(|j| {
                    let beta = 2.0 * PI * (j as f32) / (lon_segs as f32);
                    out.add_vertex(surface_at(s, beta), normal_at(s, beta), MAT_SHELL)
                })
                .collect()
        })
        .collect();

    for j in 0..lon_segs {
        let j_next = (j + 1) % lon_segs;
        out.add_tri(outer_apex, rings[0][j], rings[0][j_next]);
    }
    for i in 0..lat_segs - 1 {
        for j in 0..lon_segs {
            let j_next = (j + 1) % lon_segs;
            out.add_quad(
                rings[i][j],
                rings[i + 1][j],
                rings[i + 1][j_next],
                rings[i][j_next],
            );
        }
    }
    let last = &rings[lat_segs - 1];
    for j in 0..lon_segs {
        let j_next = (j + 1) % lon_segs;
        out.add_tri(last[j], inner_apex, last[j_next]);
    }
}

fn add_touch_accent_ring(
    out: &mut MeshBuffer,
    surface_at: &dyn Fn(f32, f32) -> Vec3,
    normal_at: &dyn Fn(f32, f32) -> Vec3,
) {
    let major_segs = 44usize;
    let minor_segs = 14usize;
    let s_ring = 0.68;
    let tube_out = 0.011;
    let tube_rad = 0.015;
    let eps = 0.002;

    let grid: Vec<Vec<u32>> = (0..major_segs)
        .map(|i| {
            let beta = 2.0 * PI * (i as f32) / (major_segs as f32);
            let surf_norm = normal_at(s_ring, beta);
            let ring_center = surface_at(s_ring, beta) + surf_norm * 0.003;
            let tangent =
                (surface_at(s_ring, beta + eps) - surface_at(s_ring, beta - eps)).normalized();
            let radial_out = tangent.cross(surf_norm).normalized();

            (0..minor_segs)
                .map(|j| {
                    let v = 2.0 * PI * (j as f32) / (minor_segs as f32);
                    let cv = v.cos();
                    let sv = v.sin();
                    let pos = ring_center + radial_out * (tube_rad * cv) + surf_norm * (tube_out * sv);
                    let norm = (radial_out * (cv / tube_rad) + surf_norm * (sv / tube_out)).normalized();
                    out.add_vertex(pos, norm, MAT_METAL)
                })
                .collect()
        })
        .collect();

    for i in 0..major_segs {
        let i_next = (i + 1) % major_segs;
        for j in 0..minor_segs {
            let j_next = (j + 1) % minor_segs;
            out.add_tri(grid[i][j], grid[i_next][j], grid[i_next][j_next]);
            out.add_tri(grid[i][j], grid[i_next][j_next], grid[i][j_next]);
        }
    }
}

fn add_earbud_mic_port(out: &mut MeshBuffer, surface_pt: Vec3, surf_norm: Vec3) {
    let b_start = surface_pt - surf_norm * 0.008;
    let b_end = surface_pt + surf_norm * 0.006;
    add_swept_bezier_tube(
        out,
        b_start,
        surface_pt,
        surface_pt,
        b_end,
        0.030,
        0.024,
        3,
        16,
        MAT_METAL,
    );

    let axis_a = initial_perp(surf_norm);
    let axis_b = surf_norm.cross(axis_a).normalized();
    let segs = 16usize;
    let disc_center = b_end + surf_norm * 0.001;
    let center_idx = out.add_vertex(disc_center, surf_norm, MAT_SCRIM);
    let ring: Vec<u32> = (0..segs)
        .map(|j| {
            let phi = 2.0 * PI * (j as f32) / (segs as f32);
            let pos = disc_center + (axis_a * phi.cos() + axis_b * phi.sin()) * 0.017;
            out.add_vertex(pos, surf_norm, MAT_SCRIM)
        })
        .collect();
    for j in 0..segs {
        let j_next = (j + 1) % segs;
        out.add_tri(center_idx, ring[j], ring[j_next]);
    }
}

fn add_silicone_ear_tip(out: &mut MeshBuffer, tip_base: Vec3, dir: Vec3) {
    let axis_a = initial_perp(dir);
    let axis_b = dir.cross(axis_a).normalized();

    let profile_r = [
        0.056, 0.055, 0.054, 0.054, 0.058, 0.068, 0.082, 0.097, 0.113, 0.128, 0.140, 0.147, 0.148,
        0.143, 0.133, 0.122, 0.112, 0.108, 0.100, 0.086, 0.070, 0.058,
    ];
    let profile_z = [
        0.000, 0.035, 0.075, 0.112, 0.125, 0.134, 0.135, 0.128, 0.114, 0.094, 0.069, 0.042, 0.015,
        -0.010, -0.030, -0.036, -0.030, -0.014, 0.004, 0.016, 0.012, 0.002,
    ];

    let prof_count = profile_r.len();
    let phi_segs = 36usize;

    let grid: Vec<Vec<u32>> = (0..prof_count)
        .map(|i| {
            let i_prev = (i + prof_count - 1) % prof_count;
            let i_next = (i + 1) % prof_count;
            let dr = profile_r[i_next] - profile_r[i_prev];
            let dz = profile_z[i_next] - profile_z[i_prev];
            let r = profile_r[i];
            let z = profile_z[i];

            (0..phi_segs)
                .map(|j| {
                    let phi = 2.0 * PI * (j as f32) / (phi_segs as f32);
                    let cp = phi.cos();
                    let sp = phi.sin();
                    let radial = axis_a * cp + axis_b * sp;
                    let t_phi = axis_a * (-sp) + axis_b * cp;
                    let pos = tip_base + radial * r + dir * z;
                    let t_prof = radial * dr + dir * dz;
                    let norm = t_prof.cross(t_phi).normalized();
                    out.add_vertex(pos, norm, MAT_CUSHION)
                })
                .collect()
        })
        .collect();

    for i in 0..prof_count {
        let i_next = (i + 1) % prof_count;
        for j in 0..phi_segs {
            let j_next = (j + 1) % phi_segs;
            out.add_quad(
                grid[i][j],
                grid[i_next][j],
                grid[i_next][j_next],
                grid[i][j_next],
            );
        }
    }
}

fn add_nozzle_acoustic_scrim(out: &mut MeshBuffer, tip_base: Vec3, dir: Vec3) {
    let axis_a = initial_perp(dir);
    let axis_b = dir.cross(axis_a).normalized();
    let segs = 24usize;

    let disc_center = tip_base + dir * 0.090;
    let center_idx = out.add_vertex(disc_center, dir, MAT_SCRIM);
    let ring: Vec<u32> = (0..segs)
        .map(|j| {
            let phi = 2.0 * PI * (j as f32) / (segs as f32);
            let pos = tip_base + dir * 0.094 + (axis_a * phi.cos() + axis_b * phi.sin()) * 0.0545;
            out.add_vertex(pos, dir, MAT_SCRIM)
        })
        .collect();
    for j in 0..segs {
        let j_next = (j + 1) % segs;
        out.add_tri(center_idx, ring[j], ring[j_next]);
    }

    let bore_r = 0.0535;
    let z0 = 0.092;
    let z1 = 0.116;
    let r0: Vec<u32> = (0..segs)
        .map(|j| {
            let phi = 2.0 * PI * (j as f32) / (segs as f32);
            let radial = axis_a * phi.cos() + axis_b * phi.sin();
            out.add_vertex(tip_base + dir * z0 + radial * bore_r, radial * -1.0, MAT_SCRIM)
        })
        .collect();
    let r1: Vec<u32> = (0..segs)
        .map(|j| {
            let phi = 2.0 * PI * (j as f32) / (segs as f32);
            let radial = axis_a * phi.cos() + axis_b * phi.sin();
            out.add_vertex(tip_base + dir * z1 + radial * bore_r, radial * -1.0, MAT_SCRIM)
        })
        .collect();
    for j in 0..segs {
        let j_next = (j + 1) % segs;
        out.add_quad(r0[j], r1[j], r1[j_next], r0[j_next]);
    }
}

fn add_headband_arch(out: &mut MeshBuffer) {
    let len_segs = 36usize;
    let rad_segs = 20usize;
    let max_angle = 76.0_f32.to_radians();
    let rx = 0.725;
    let ry = 0.80;
    let cy = -0.02;

    let rings: Vec<Vec<u32>> = (0..=len_segs)
        .map(|s| {
            let u = -1.0 + 2.0 * (s as f32) / (len_segs as f32);
            let theta = u * max_angle;
            let sin_t = theta.sin();
            let cos_t = theta.cos();
            let center = Vec3::new(rx * sin_t, cy + ry * cos_t, 0.0);
            let n_in = Vec3::new(-ry * sin_t, -rx * cos_t, 0.0).normalized();
            let binorm = Vec3::new(0.0, 0.0, 1.0);

            let u2 = u * u;
            let r_thick = 0.036 - 0.007 * u2;
            let r_width = 0.072 - 0.034 * u2;

            (0..rad_segs)
                .map(|p| {
                    let phi = 2.0 * PI * (p as f32) / (rad_segs as f32);
                    let cp = phi.cos();
                    let sp = phi.sin();
                    let pos = center + n_in * (r_thick * cp) + binorm * (r_width * sp);
                    let norm = (n_in * (cp / r_thick) + binorm * (sp / r_width)).normalized();
                    out.add_vertex(pos, norm, MAT_SHELL)
                })
                .collect()
        })
        .collect();

    stitch_closed_tube(out, &rings, true, true, MAT_SHELL);
}

fn add_headband_crown_cushion(out: &mut MeshBuffer) {
    let len_segs = 24usize;
    let rad_segs = 14usize;
    let max_angle = 60.0_f32.to_radians();
    let rx = 0.725;
    let ry = 0.80;
    let cy = -0.02;

    let rings: Vec<Vec<u32>> = (0..=len_segs)
        .map(|s| {
            let u = -1.0 + 2.0 * (s as f32) / (len_segs as f32);
            let theta = u * max_angle;
            let sin_t = theta.sin();
            let cos_t = theta.cos();
            let n_in = Vec3::new(-ry * sin_t, -rx * cos_t, 0.0).normalized();
            let binorm = Vec3::new(0.0, 0.0, 1.0);
            let center = Vec3::new(rx * sin_t, cy + ry * cos_t, 0.0) + n_in * 0.024;

            let fade = (1.0 - u * u * u * u).clamp(0.28, 1.0);
            let r_thick = 0.028 * fade;
            let r_width = 0.056 * fade;

            (0..rad_segs)
                .map(|p| {
                    let phi = 2.0 * PI * (p as f32) / (rad_segs as f32);
                    let cp = phi.cos();
                    let sp = phi.sin();
                    let pos = center + n_in * (r_thick * cp) + binorm * (r_width * sp);
                    let norm = (n_in * (cp / r_thick) + binorm * (sp / r_width)).normalized();
                    out.add_vertex(pos, norm, MAT_CUSHION)
                })
                .collect()
        })
        .collect();

    stitch_closed_tube(out, &rings, true, true, MAT_CUSHION);
}

fn build_right_side_assembly(side: &mut MeshBuffer) {
    let tip_theta = 76.0_f32.to_radians();
    let tip_center = Vec3::new(0.725 * tip_theta.sin(), -0.02 + 0.80 * tip_theta.cos(), 0.0);
    let tip_dir = Vec3::new(0.725 * tip_theta.cos(), -0.80 * tip_theta.sin(), 0.0).normalized();

    let collar_start = tip_center - tip_dir * 0.012;
    let collar_end = tip_center + tip_dir * 0.036;
    add_swept_bezier_tube(
        side,
        collar_start,
        collar_start + tip_dir * 0.016,
        collar_end - tip_dir * 0.016,
        collar_end,
        0.041,
        0.038,
        4,
        16,
        MAT_METAL,
    );

    let stem_start = tip_center + tip_dir * 0.015;
    let stem_end = Vec3::new(stem_start.x + 0.026, -0.055, 0.0);
    add_swept_bezier_tube(
        side,
        stem_start,
        stem_start + Vec3::new(0.01, -0.07, 0.0),
        stem_end + Vec3::new(-0.005, 0.06, 0.0),
        stem_end,
        0.028,
        0.028,
        6,
        16,
        MAT_METAL,
    );

    let cup_center = Vec3::new(0.585, -0.355, 0.0);
    let cant_rad = (-10.0_f32).to_radians();
    let toe_rad = 7.0_f32.to_radians();

    let cup_out = rotate_y(rotate_z(Vec3::new(1.0, 0.0, 0.0), cant_rad), toe_rad).normalized();
    let cup_up = rotate_y(rotate_z(Vec3::new(0.0, 1.0, 0.0), cant_rad), toe_rad).normalized();
    let cup_fwd = cup_out.cross(cup_up).normalized();

    let arm_start = Vec3::new(stem_end.x, stem_end.y + 0.018, 0.0);
    let arm_target = cup_center + cup_up * 0.135 + cup_out * 0.095;
    add_swept_bezier_tube(
        side,
        arm_start,
        Vec3::new(arm_start.x + 0.012, arm_start.y - 0.065, 0.0),
        arm_target + Vec3::new(0.035, 0.075, 0.01),
        arm_target,
        0.038,
        0.046,
        10,
        16,
        MAT_SHELL,
    );

    let r_up = 0.375;
    let r_fwd = 0.285;
    let dome_depth = 0.165;
    add_earcup_dome(side, cup_center, cup_out, cup_up, cup_fwd, r_up, r_fwd, dome_depth);

    add_mic_bezel(side, cup_center, cup_out, cup_up, cup_fwd, r_up, r_fwd, dome_depth, 122.0);
    add_mic_bezel(side, cup_center, cup_out, cup_up, cup_fwd, r_up, r_fwd, dome_depth, -58.0);

    add_oval_ear_cushion(
        side,
        cup_center - cup_out * 0.052,
        cup_out,
        cup_up,
        cup_fwd,
        r_up - 0.048,
        r_fwd - 0.042,
        0.080,
        0.068,
    );

    add_recessed_baffle(
        side,
        cup_center - cup_out * 0.025,
        cup_out,
        cup_up,
        cup_fwd,
        r_up - 0.065,
        r_fwd - 0.06,
    );
}

#[allow(clippy::too_many_arguments)]
fn add_earcup_dome(
    out: &mut MeshBuffer,
    center: Vec3,
    cup_out: Vec3,
    cup_up: Vec3,
    cup_fwd: Vec3,
    r_up: f32,
    r_fwd: f32,
    dome_depth: f32,
) {
    let lat_segs = 20usize;
    let lon_segs = 36usize;
    let max_s = PI * 0.5 + 0.14;

    let profile = |s: f32| -> (f32, f32) {
        if s <= PI * 0.5 {
            let ss = s.sin();
            let cc = s.cos();
            let rad = ss;
            let axial = dome_depth * cc * (1.0 + 0.34 * ss * ss);
            (rad, axial)
        } else {
            let extra = s - PI * 0.5;
            (1.0 - extra * 0.28, -extra * 0.19)
        }
    };

    let apex_idx = out.add_vertex(center + cup_out * dome_depth, cup_out, MAT_SHELL);

    let rings: Vec<Vec<u32>> = (0..lat_segs)
        .map(|i| {
            let t = (i + 1) as f32 / lat_segs as f32;
            let s = t * max_s;
            let (rad, axial) = profile(s);
            let eps = 0.002;
            let (r0, w0) = profile((s - eps).max(0.0));
            let (r1, w1) = profile(s + eps);
            let dr = r1 - r0;
            let dw = w1 - w0;

            (0..lon_segs)
                .map(|j| {
                    let beta = 2.0 * PI * (j as f32) / (lon_segs as f32);
                    let cb = beta.cos();
                    let sb = beta.sin();
                    let radial_vec = cup_up * (r_up * rad * cb) + cup_fwd * (r_fwd * rad * sb);
                    let pos = center + radial_vec + cup_out * axial;

                    let t_lat = cup_up * (r_up * dr * cb) + cup_fwd * (r_fwd * dr * sb) + cup_out * dw;
                    let t_lon = cup_up * (-r_up * sb) + cup_fwd * (r_fwd * cb);
                    let norm = t_lat.cross(t_lon).normalized();
                    out.add_vertex(pos, norm, MAT_SHELL)
                })
                .collect()
        })
        .collect();

    for j in 0..lon_segs {
        let j_next = (j + 1) % lon_segs;
        out.add_tri(apex_idx, rings[0][j], rings[0][j_next]);
    }
    for i in 0..lat_segs - 1 {
        for j in 0..lon_segs {
            let j_next = (j + 1) % lon_segs;
            out.add_quad(
                rings[i][j],
                rings[i + 1][j],
                rings[i + 1][j_next],
                rings[i][j_next],
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn add_mic_bezel(
    out: &mut MeshBuffer,
    center: Vec3,
    cup_out: Vec3,
    cup_up: Vec3,
    cup_fwd: Vec3,
    r_up: f32,
    r_fwd: f32,
    dome_depth: f32,
    beta_deg: f32,
) {
    let s = 0.92_f32;
    let ss = s.sin();
    let cc = s.cos();
    let axial = dome_depth * cc * (1.0 + 0.34 * ss * ss);
    let beta = beta_deg.to_radians();
    let cb = beta.cos();
    let sb = beta.sin();

    let surface_pt =
        center + cup_up * (r_up * ss * cb) + cup_fwd * (r_fwd * ss * sb) + cup_out * axial;
    let surf_norm =
        (cup_up * (cb / r_up) + cup_fwd * (sb / r_fwd) + cup_out * (0.8 / dome_depth)).normalized();

    let b_start = surface_pt - surf_norm * 0.008;
    let b_end = surface_pt + surf_norm * 0.006;
    add_swept_bezier_tube(
        out,
        b_start,
        surface_pt,
        surface_pt,
        b_end,
        0.022,
        0.017,
        2,
        12,
        MAT_METAL,
    );
}

#[allow(clippy::too_many_arguments)]
fn add_oval_ear_cushion(
    out: &mut MeshBuffer,
    cup_center: Vec3,
    cup_out: Vec3,
    cup_up: Vec3,
    cup_fwd: Vec3,
    ring_up: f32,
    ring_fwd: f32,
    tube_out: f32,
    tube_rad: f32,
) {
    let major_segs = 36usize;
    let minor_segs = 18usize;

    let grid: Vec<Vec<u32>> = (0..major_segs)
        .map(|i| {
            let u = 2.0 * PI * (i as f32) / (major_segs as f32);
            let cu = u.cos();
            let su = u.sin();
            let ring_center = cup_center + cup_up * (ring_up * cu) + cup_fwd * (ring_fwd * su);
            let tangent = (cup_up * (-ring_up * su) + cup_fwd * (ring_fwd * cu)).normalized();
            let radial_out = tangent.cross(cup_out).normalized();

            (0..minor_segs)
                .map(|j| {
                    let v = 2.0 * PI * (j as f32) / (minor_segs as f32);
                    let cv = v.cos();
                    let sv = v.sin();
                    let pos = ring_center + radial_out * (tube_rad * cv) + cup_out * (tube_out * sv);
                    let norm = (radial_out * (cv / tube_rad) + cup_out * (sv / tube_out)).normalized();
                    out.add_vertex(pos, norm, MAT_CUSHION)
                })
                .collect()
        })
        .collect();

    for i in 0..major_segs {
        let i_next = (i + 1) % major_segs;
        for j in 0..minor_segs {
            let j_next = (j + 1) % minor_segs;
            out.add_tri(grid[i][j], grid[i_next][j], grid[i_next][j_next]);
            out.add_tri(grid[i][j], grid[i_next][j_next], grid[i][j_next]);
        }
    }
}

fn add_recessed_baffle(
    out: &mut MeshBuffer,
    cup_center: Vec3,
    cup_out: Vec3,
    cup_up: Vec3,
    cup_fwd: Vec3,
    r_up: f32,
    r_fwd: f32,
) {
    let segs = 24usize;
    let inward_norm = cup_out * -1.0;
    let center_idx = out.add_vertex(cup_center - cup_out * 0.018, inward_norm, MAT_SCRIM);
    let ring: Vec<u32> = (0..segs)
        .map(|j| {
            let beta = 2.0 * PI * (j as f32) / (segs as f32);
            let pos = cup_center + cup_up * (r_up * beta.cos()) + cup_fwd * (r_fwd * beta.sin());
            out.add_vertex(pos, inward_norm, MAT_SCRIM)
        })
        .collect();
    for j in 0..segs {
        let j_next = (j + 1) % segs;
        out.add_tri(center_idx, ring[j_next], ring[j]);
    }
}

#[allow(clippy::too_many_arguments)]
fn add_swept_bezier_tube(
    out: &mut MeshBuffer,
    p0: Vec3,
    p1: Vec3,
    p2: Vec3,
    p3: Vec3,
    r_start: f32,
    r_end: f32,
    len_segs: usize,
    rad_segs: usize,
    mat_id: f32,
) {
    let mut prev_t = bezier_tangent(p0, p1, p2, p3, 0.0).normalized();
    let mut prev_n = initial_perp(prev_t);

    let rings: Vec<Vec<u32>> = (0..=len_segs)
        .map(|s| {
            let t = s as f32 / len_segs as f32;
            let center = bezier_point(p0, p1, p2, p3, t);
            let tang = bezier_tangent(p0, p1, p2, p3, t).normalized();
            let norm = parallel_transport(prev_n, prev_t, tang);
            let binorm = norm.cross(tang).normalized();
            prev_t = tang;
            prev_n = norm;

            let r = r_start + (r_end - r_start) * t;
            (0..rad_segs)
                .map(|p| {
                    let phi = 2.0 * PI * (p as f32) / (rad_segs as f32);
                    let dir = norm * phi.cos() + binorm * phi.sin();
                    out.add_vertex(center + dir * r, dir, mat_id)
                })
                .collect()
        })
        .collect();

    stitch_closed_tube(out, &rings, true, true, mat_id);
}

fn stitch_closed_tube(
    out: &mut MeshBuffer,
    rings: &[Vec<u32>],
    cap_start: bool,
    cap_end: bool,
    mat_id: f32,
) {
    let len = rings.len();
    let rad = rings[0].len();
    for s in 0..len - 1 {
        for p in 0..rad {
            let p_next = (p + 1) % rad;
            out.add_quad(
                rings[s][p],
                rings[s + 1][p],
                rings[s + 1][p_next],
                rings[s][p_next],
            );
        }
    }
    if cap_start {
        add_flat_cap(out, &rings[0], false, mat_id);
    }
    if cap_end {
        add_flat_cap(out, &rings[len - 1], true, mat_id);
    }
}

fn add_flat_cap(out: &mut MeshBuffer, ring: &[u32], flip: bool, mat_id: f32) {
    let (mut sx, mut sy, mut sz) = (0.0, 0.0, 0.0);
    for &idx in ring {
        let o = idx as usize * STRIDE_FLOATS;
        sx += out.verts[o];
        sy += out.verts[o + 1];
        sz += out.verts[o + 2];
    }
    let inv = 1.0 / ring.len() as f32;
    let center = Vec3::new(sx * inv, sy * inv, sz * inv);

    let o0 = ring[0] as usize * STRIDE_FLOATS;
    let o1 = ring[1] as usize * STRIDE_FLOATS;
    let p0 = Vec3::new(out.verts[o0], out.verts[o0 + 1], out.verts[o0 + 2]);
    let p1 = Vec3::new(out.verts[o1], out.verts[o1 + 1], out.verts[o1 + 2]);
    let raw_n = (p0 - center).cross(p1 - center).normalized();
    let cap_n = if flip { raw_n * -1.0 } else { raw_n };

    let center_idx = out.add_vertex(center, cap_n, mat_id);
    let cap_verts: Vec<u32> = ring
        .iter()
        .map(|&idx| {
            let o = idx as usize * STRIDE_FLOATS;
            out.add_vertex(
                Vec3::new(out.verts[o], out.verts[o + 1], out.verts[o + 2]),
                cap_n,
                mat_id,
            )
        })
        .collect();

    for k in 0..ring.len() {
        let k_next = (k + 1) % ring.len();
        if flip {
            out.add_tri(center_idx, cap_verts[k_next], cap_verts[k]);
        } else {
            out.add_tri(center_idx, cap_verts[k], cap_verts[k_next]);
        }
    }
}

fn bezier_point(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    let u = 1.0 - t;
    p0 * (u * u * u) + p1 * (3.0 * u * u * t) + p2 * (3.0 * u * t * t) + p3 * (t * t * t)
}

fn bezier_tangent(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    let u = 1.0 - t;
    (p1 - p0) * (3.0 * u * u) + (p2 - p1) * (6.0 * u * t) + (p3 - p2) * (3.0 * t * t)
}

fn initial_perp(t: Vec3) -> Vec3 {
    let ref_v = if t.z.abs() < 0.85 {
        Vec3::new(0.0, 0.0, 1.0)
    } else {
        Vec3::new(1.0, 0.0, 0.0)
    };
    ref_v.cross(t).normalized()
}

fn parallel_transport(prev_n: Vec3, prev_t: Vec3, next_t: Vec3) -> Vec3 {
    let axis = prev_t.cross(next_t);
    let len = axis.length();
    if len < 1e-5 {
        return prev_n;
    }
    let k = axis * (1.0 / len);
    let dot = (prev_t.x * next_t.x + prev_t.y * next_t.y + prev_t.z * next_t.z).clamp(-1.0, 1.0);
    let angle = dot.acos();
    let (sa, ca) = angle.sin_cos();
    let k_cross_n = k.cross(prev_n);
    let k_dot_n = k.x * prev_n.x + k.y * prev_n.y + k.z * prev_n.z;
    (prev_n * ca + k_cross_n * sa + k * (k_dot_n * (1.0 - ca))).normalized()
}

fn rotate_z(v: Vec3, rad: f32) -> Vec3 {
    let (s, c) = rad.sin_cos();
    Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
}

fn rotate_y(v: Vec3, rad: f32) -> Vec3 {
    let (s, c) = rad.sin_cos();
    Vec3::new(v.x * c + v.z * s, v.y, -v.x * s + v.z * c)
}
