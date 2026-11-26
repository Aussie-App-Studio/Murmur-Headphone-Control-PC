use std::sync::{Arc, Mutex};
use std::time::Instant;

use egui::{Color32, Pos2, Sense, Shape, Stroke, Ui, Vec2};
use glow::HasContext;

use crate::catalog::HeadphoneFormFactor;
use crate::models::HeadphoneFinish;
use crate::ui::headphone_mesh::{build_earbuds_mesh, build_xm5_mesh, STRIDE_FLOATS};
use crate::ui::theme::MurmurPalette;
use crate::ui::widgets::color_alpha;

struct GlMeshBuffers {
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    ibo: glow::Buffer,
    index_count: i32,
}

struct ShadowCache {
    key: Option<(i32, i32, i32, i32, bool, bool, Color32)>,
    shapes: Vec<Shape>,
}

struct GlInner {
    program: glow::Program,
    over_ear: GlMeshBuffers,
    earbuds: GlMeshBuffers,
    u_mvp: Option<glow::UniformLocation>,
    u_model: Option<glow::UniformLocation>,
    u_normal_mat: Option<glow::UniformLocation>,
    u_shell_color: Option<glow::UniformLocation>,
    u_cushion_color: Option<glow::UniformLocation>,
    u_metal_color: Option<glow::UniformLocation>,
    u_aura_color: Option<glow::UniformLocation>,
    u_cam_pos: Option<glow::UniformLocation>,
    u_dim: Option<glow::UniformLocation>,
    u_is_dark: Option<glow::UniformLocation>,

    // Smoothed state
    cur_shell: [f32; 3],
    cur_cushion: [f32; 3],
    cur_metal: [f32; 3],
    cur_aura: [f32; 3],
    cur_dim: f32,
    cur_spin: f32,
    cur_dark: f32,

    // Targets
    target_shell: [f32; 3],
    target_cushion: [f32; 3],
    target_metal: [f32; 3],
    target_aura: [f32; 3],
    target_dim: f32,
    target_spin: f32,
    target_dark: f32,
    form_factor: HeadphoneFormFactor,

    // Kinematics
    yaw_deg: f32,
    drag_velocity_deg: f32,
    pending_drag_deg: f32,
    bob_phase: f32,
    last_frame: Instant,
    shadow_cache: ShadowCache,
}

pub struct HeadphoneGlStage {
    inner: Option<Arc<Mutex<GlInner>>>,
}

impl HeadphoneGlStage {
    pub fn new(gl: Option<&Arc<glow::Context>>) -> Self {
        let Some(gl) = gl else {
            return Self { inner: None };
        };
        let inner = unsafe { init_gl(gl) };
        Self {
            inner: inner.map(|g| Arc::new(Mutex::new(g))),
        }
    }

    pub fn draw(
        &self,
        ui: &mut Ui,
        p: &MurmurPalette,
        finish: HeadphoneFinish,
        form_factor: HeadphoneFormFactor,
        mode_accent: Color32,
        is_offline: bool,
        height: f32,
    ) {
        let width = ui.available_width().max(220.0);
        let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::drag());

        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }

        let Some(inner_arc) = &self.inner else {
            return;
        };

        let active_motion = {
            let mut inner = inner_arc.lock().unwrap();
            inner.form_factor = form_factor;
            inner.target_shell = rgb_u32_to_linear(finish.shell_rgb());
            inner.target_cushion = rgb_u32_to_linear(finish.cushion_rgb());
            inner.target_metal = rgb_u32_to_linear(finish.metal_rgb());
            inner.target_aura = color32_to_linear(mode_accent);
            inner.target_dim = if is_offline { 0.56 } else { 1.0 };
            inner.target_spin = if is_offline { 4.5 } else { 16.0 };
            inner.target_dark = if p.is_dark { 1.0 } else { 0.0 };

            if response.dragged() {
                inner.pending_drag_deg += response.drag_delta().x * 0.45;
            }
            response.dragged() || inner.drag_velocity_deg.abs() > 0.5
        };

        let minimized = ui
            .ctx()
            .input(|i| i.viewport().minimized.unwrap_or(false));
        let visible = ui.is_rect_visible(rect) && !minimized;

        if visible {
            let painter = ui.painter_at(rect);

            // Cached studio floor shadow + subtle mode aura reflection beneath headphones
            let shadow_center = Pos2::new(rect.center().x, rect.bottom() - 20.0);
            let rx = (rect.width() * 0.30).clamp(60.0, 125.0);
            let ry = 13.0_f32;
            let key = (
                shadow_center.x.round() as i32,
                shadow_center.y.round() as i32,
                rx.round() as i32,
                ry.round() as i32,
                p.is_dark,
                is_offline,
                mode_accent,
            );
            let shapes = {
                let mut inner = inner_arc.lock().unwrap();
                if inner.shadow_cache.key != Some(key) {
                    let mut built = Vec::with_capacity(8);
                    let shadow_base = if p.is_dark {
                        Color32::from_rgba_unmultiplied(0, 0, 0, if is_offline { 42 } else { 85 })
                    } else {
                        Color32::from_rgba_unmultiplied(
                            15,
                            23,
                            42,
                            if is_offline { 20 } else { 38 },
                        )
                    };
                    append_soft_ellipse(&mut built, shadow_center, rx, ry, shadow_base);
                    if !is_offline {
                        append_soft_ellipse(
                            &mut built,
                            Pos2::new(shadow_center.x, shadow_center.y - 1.5),
                            rx * 0.78,
                            ry * 0.65,
                            color_alpha(mode_accent, if p.is_dark { 0.16 } else { 0.10 }),
                        );
                    }
                    inner.shadow_cache.key = Some(key);
                    inner.shadow_cache.shapes = built;
                }
                inner.shadow_cache.shapes.clone()
            };
            painter.extend(shapes);

            let inner_cb = Arc::clone(inner_arc);
            let callback = egui::PaintCallback {
                rect,
                callback: Arc::new(egui_glow::CallbackFn::new(move |info, glow_painter| {
                    let gl = glow_painter.gl();
                    let mut inner = inner_cb.lock().unwrap();
                    paint_gl(gl, &mut inner, info.viewport.width(), info.viewport.height());
                })),
            };
            painter.add(callback);

            // Pace turntable animation (~60 FPS when dragging/coasting, ~30 FPS passive spin when focused; paused when unfocused/minimized)
            let focused = ui.ctx().input(|i| i.viewport().focused.unwrap_or(true));
            if focused || active_motion {
                let delay_ms = if active_motion { 16 } else { 33 };
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(delay_ms));
            }
        }
    }
}

unsafe fn init_gl(gl: &glow::Context) -> Option<GlInner> {
    let vs = gl.create_shader(glow::VERTEX_SHADER).ok()?;
    gl.shader_source(vs, VERTEX_SHADER);
    gl.compile_shader(vs);
    if !gl.get_shader_compile_status(vs) {
        log::error!("Vertex shader error: {}", gl.get_shader_info_log(vs));
        gl.delete_shader(vs);
        return None;
    }

    let fs = gl.create_shader(glow::FRAGMENT_SHADER).ok()?;
    gl.shader_source(fs, FRAGMENT_SHADER);
    gl.compile_shader(fs);
    if !gl.get_shader_compile_status(fs) {
        log::error!("Fragment shader error: {}", gl.get_shader_info_log(fs));
        gl.delete_shader(vs);
        gl.delete_shader(fs);
        return None;
    }

    let program = gl.create_program().ok()?;
    gl.attach_shader(program, vs);
    gl.attach_shader(program, fs);
    gl.link_program(program);
    gl.delete_shader(vs);
    gl.delete_shader(fs);
    if !gl.get_program_link_status(program) {
        log::error!("Shader link error: {}", gl.get_program_info_log(program));
        gl.delete_program(program);
        return None;
    }

    let over_ear = upload_mesh(gl, build_xm5_mesh())?;
    let earbuds = upload_mesh(gl, build_earbuds_mesh())?;

    let init_shell = rgb_u32_to_linear(HeadphoneFinish::ObsidianBlack.shell_rgb());
    let init_cushion = rgb_u32_to_linear(HeadphoneFinish::ObsidianBlack.cushion_rgb());
    let init_metal = rgb_u32_to_linear(HeadphoneFinish::ObsidianBlack.metal_rgb());

    Some(GlInner {
        u_mvp: gl.get_uniform_location(program, "uMvp"),
        u_model: gl.get_uniform_location(program, "uModel"),
        u_normal_mat: gl.get_uniform_location(program, "uNormalMat"),
        u_shell_color: gl.get_uniform_location(program, "uShellColor"),
        u_cushion_color: gl.get_uniform_location(program, "uCushionColor"),
        u_metal_color: gl.get_uniform_location(program, "uMetalColor"),
        u_aura_color: gl.get_uniform_location(program, "uAuraColor"),
        u_cam_pos: gl.get_uniform_location(program, "uCamPos"),
        u_dim: gl.get_uniform_location(program, "uDim"),
        u_is_dark: gl.get_uniform_location(program, "uIsDark"),
        program,
        over_ear,
        earbuds,
        cur_shell: init_shell,
        cur_cushion: init_cushion,
        cur_metal: init_metal,
        cur_aura: [0.1, 0.5, 0.45],
        cur_dim: 0.56,
        cur_spin: 4.5,
        cur_dark: 1.0,
        target_shell: init_shell,
        target_cushion: init_cushion,
        target_metal: init_metal,
        target_aura: [0.1, 0.5, 0.45],
        target_dim: 0.56,
        target_spin: 4.5,
        target_dark: 1.0,
        form_factor: HeadphoneFormFactor::OverEar,
        yaw_deg: 28.0,
        drag_velocity_deg: 0.0,
        pending_drag_deg: 0.0,
        bob_phase: 0.0,
        last_frame: Instant::now(),
        shadow_cache: ShadowCache {
            key: None,
            shapes: Vec::new(),
        },
    })
}

unsafe fn upload_mesh(
    gl: &glow::Context,
    mesh: crate::ui::headphone_mesh::MeshData,
) -> Option<GlMeshBuffers> {
    let vao = gl.create_vertex_array().ok()?;
    let vbo = gl.create_buffer().ok()?;
    let ibo = gl.create_buffer().ok()?;

    gl.bind_vertex_array(Some(vao));

    gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
    gl.buffer_data_u8_slice(
        glow::ARRAY_BUFFER,
        bytemuck::cast_slice(&mesh.vertices),
        glow::STATIC_DRAW,
    );

    gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ibo));
    gl.buffer_data_u8_slice(
        glow::ELEMENT_ARRAY_BUFFER,
        bytemuck::cast_slice(&mesh.indices),
        glow::STATIC_DRAW,
    );

    let stride = (STRIDE_FLOATS * std::mem::size_of::<f32>()) as i32;
    gl.enable_vertex_attrib_array(0);
    gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
    gl.enable_vertex_attrib_array(1);
    gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, stride, 3 * 4);
    gl.enable_vertex_attrib_array(2);
    gl.vertex_attrib_pointer_f32(2, 1, glow::FLOAT, false, stride, 6 * 4);

    gl.bind_vertex_array(None);

    Some(GlMeshBuffers {
        vao,
        vbo,
        ibo,
        index_count: mesh.indices.len() as i32,
    })
}

fn paint_gl(gl: &glow::Context, s: &mut GlInner, w_pts: f32, h_pts: f32) {
    let now = Instant::now();
    let dt = (now - s.last_frame).as_secs_f32().clamp(0.001, 0.05);
    s.last_frame = now;

    let rate = 1.0 - (-6.0 * dt).exp();
    lerp_vec3(&mut s.cur_shell, &s.target_shell, rate);
    lerp_vec3(&mut s.cur_cushion, &s.target_cushion, rate);
    lerp_vec3(&mut s.cur_metal, &s.target_metal, rate);
    lerp_vec3(&mut s.cur_aura, &s.target_aura, rate);
    s.cur_dim += (s.target_dim - s.cur_dim) * rate;
    s.cur_spin += (s.target_spin - s.cur_spin) * rate;
    s.cur_dark += (s.target_dark - s.cur_dark) * rate;

    if s.pending_drag_deg != 0.0 {
        s.yaw_deg += s.pending_drag_deg;
        s.drag_velocity_deg = (s.pending_drag_deg / dt).clamp(-260.0, 260.0);
        s.pending_drag_deg = 0.0;
    }
    s.yaw_deg = (s.yaw_deg + (s.cur_spin + s.drag_velocity_deg) * dt) % 360.0;
    s.drag_velocity_deg *= (-4.8 * dt).exp();
    s.bob_phase += dt * 1.35;

    let bob_y = s.bob_phase.sin() * 0.026;
    let pitch_breath = (s.bob_phase * 0.72).sin() * 2.4 - 4.0;
    let roll_tilt = (s.bob_phase * 0.51).sin() * 1.8;

    let aspect = (w_pts / h_pts.max(1.0)).max(0.2);
    let proj = mat4_perspective(38.0_f32.to_radians(), aspect, 0.5, 12.0);
    let cam = [0.0_f32, 0.22, 3.15];
    let view = mat4_look_at(cam, [0.0, -0.01, 0.0], [0.0, 1.0, 0.0]);

    let mut model = mat4_translate(0.0, bob_y, 0.0);
    model = mat4_mul(&model, &mat4_rotate_x(pitch_breath.to_radians()));
    model = mat4_mul(&model, &mat4_rotate_y(s.yaw_deg.to_radians()));
    model = mat4_mul(&model, &mat4_rotate_z(roll_tilt.to_radians()));

    let mv = mat4_mul(&view, &model);
    let mvp = mat4_mul(&proj, &mv);
    let normal_mat = [
        model[0], model[1], model[2], model[4], model[5], model[6], model[8], model[9], model[10],
    ];

    let mesh = match s.form_factor {
        HeadphoneFormFactor::Earbuds => &s.earbuds,
        HeadphoneFormFactor::OverEar => &s.over_ear,
    };

    unsafe {
        gl.enable(glow::DEPTH_TEST);
        gl.depth_mask(true);
        gl.depth_func(glow::LESS);
        gl.clear_depth_f32(1.0);
        gl.clear(glow::DEPTH_BUFFER_BIT);

        gl.enable(glow::CULL_FACE);
        gl.cull_face(glow::BACK);
        gl.front_face(glow::CCW);
        gl.disable(glow::BLEND);

        gl.use_program(Some(s.program));
        gl.uniform_matrix_4_f32_slice(s.u_mvp.as_ref(), false, &mvp);
        gl.uniform_matrix_4_f32_slice(s.u_model.as_ref(), false, &model);
        gl.uniform_matrix_3_f32_slice(s.u_normal_mat.as_ref(), false, &normal_mat);
        gl.uniform_3_f32_slice(s.u_shell_color.as_ref(), &s.cur_shell);
        gl.uniform_3_f32_slice(s.u_cushion_color.as_ref(), &s.cur_cushion);
        gl.uniform_3_f32_slice(s.u_metal_color.as_ref(), &s.cur_metal);
        gl.uniform_3_f32_slice(s.u_aura_color.as_ref(), &s.cur_aura);
        gl.uniform_3_f32(s.u_cam_pos.as_ref(), cam[0], cam[1], cam[2]);
        gl.uniform_1_f32(s.u_dim.as_ref(), s.cur_dim);
        gl.uniform_1_f32(s.u_is_dark.as_ref(), s.cur_dark);

        gl.bind_vertex_array(Some(mesh.vao));
        gl.draw_elements(glow::TRIANGLES, mesh.index_count, glow::UNSIGNED_INT, 0);
        gl.bind_vertex_array(None);
        gl.use_program(None);

        gl.disable(glow::CULL_FACE);
        gl.disable(glow::DEPTH_TEST);
        gl.enable(glow::BLEND);
    }
}

fn append_soft_ellipse(
    out: &mut Vec<Shape>,
    center: Pos2,
    rx: f32,
    ry: f32,
    color: Color32,
) {
    for i in (1..=4).rev() {
        let frac = i as f32 / 4.0;
        let alpha = (1.0 - frac * 0.7).powi(2) * 0.65;
        let pts: Vec<Pos2> = (0..28)
            .map(|k| {
                let t = (k as f32 / 28.0) * std::f32::consts::TAU;
                Pos2::new(center.x + rx * frac * t.cos(), center.y + ry * frac * t.sin())
            })
            .collect();
        out.push(Shape::convex_polygon(
            pts,
            color_alpha(color, alpha),
            Stroke::NONE,
        ));
    }
}

fn rgb_u32_to_linear(rgb: u32) -> [f32; 3] {
    [
        (((rgb >> 16) & 0xFF) as f32 / 255.0).powf(2.2),
        (((rgb >> 8) & 0xFF) as f32 / 255.0).powf(2.2),
        ((rgb & 0xFF) as f32 / 255.0).powf(2.2),
    ]
}

fn color32_to_linear(c: Color32) -> [f32; 3] {
    [
        (c.r() as f32 / 255.0).powf(2.2),
        (c.g() as f32 / 255.0).powf(2.2),
        (c.b() as f32 / 255.0).powf(2.2),
    ]
}

fn lerp_vec3(cur: &mut [f32; 3], dst: &[f32; 3], alpha: f32) {
    cur[0] += (dst[0] - cur[0]) * alpha;
    cur[1] += (dst[1] - cur[1]) * alpha;
    cur[2] += (dst[2] - cur[2]) * alpha;
}

// MARK: - Column-major 4x4 Matrix Helpers

fn mat4_mul(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut out = [0.0_f32; 16];
    for col in 0..4 {
        for row in 0..4 {
            out[col * 4 + row] = a[row] * b[col * 4]
                + a[4 + row] * b[col * 4 + 1]
                + a[8 + row] * b[col * 4 + 2]
                + a[12 + row] * b[col * 4 + 3];
        }
    }
    out
}

fn mat4_translate(x: f32, y: f32, z: f32) -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        x,   y,   z,   1.0,
    ]
}

fn mat4_rotate_x(rad: f32) -> [f32; 16] {
    let (s, c) = rad.sin_cos();
    [
        1.0, 0.0, 0.0, 0.0,
        0.0, c,   s,   0.0,
        0.0, -s,  c,   0.0,
        0.0, 0.0, 0.0, 1.0,
    ]
}

fn mat4_rotate_y(rad: f32) -> [f32; 16] {
    let (s, c) = rad.sin_cos();
    [
        c,   0.0, -s,  0.0,
        0.0, 1.0, 0.0, 0.0,
        s,   0.0, c,   0.0,
        0.0, 0.0, 0.0, 1.0,
    ]
}

fn mat4_rotate_z(rad: f32) -> [f32; 16] {
    let (s, c) = rad.sin_cos();
    [
        c,   s,   0.0, 0.0,
        -s,  c,   0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ]
}

fn mat4_perspective(fovy_rad: f32, aspect: f32, near: f32, far: f32) -> [f32; 16] {
    let f = 1.0 / (fovy_rad * 0.5).tan();
    let nf = 1.0 / (near - far);
    [
        f / aspect, 0.0, 0.0, 0.0,
        0.0,        f,   0.0, 0.0,
        0.0,        0.0, (far + near) * nf, -1.0,
        0.0,        0.0, 2.0 * far * near * nf, 0.0,
    ]
}

fn mat4_look_at(eye: [f32; 3], center: [f32; 3], up: [f32; 3]) -> [f32; 16] {
    let fx = center[0] - eye[0];
    let fy = center[1] - eye[1];
    let fz = center[2] - eye[2];
    let fl = (fx * fx + fy * fy + fz * fz).sqrt().max(1e-6);
    let f = [fx / fl, fy / fl, fz / fl];

    let sx = f[1] * up[2] - f[2] * up[1];
    let sy = f[2] * up[0] - f[0] * up[2];
    let sz = f[0] * up[1] - f[1] * up[0];
    let sl = (sx * sx + sy * sy + sz * sz).sqrt().max(1e-6);
    let s = [sx / sl, sy / sl, sz / sl];

    let u = [
        s[1] * f[2] - s[2] * f[1],
        s[2] * f[0] - s[0] * f[2],
        s[0] * f[1] - s[1] * f[0],
    ];

    let tx = -(s[0] * eye[0] + s[1] * eye[1] + s[2] * eye[2]);
    let ty = -(u[0] * eye[0] + u[1] * eye[1] + u[2] * eye[2]);
    let tz = f[0] * eye[0] + f[1] * eye[1] + f[2] * eye[2];

    [
        s[0], u[0], -f[0], 0.0,
        s[1], u[1], -f[1], 0.0,
        s[2], u[2], -f[2], 0.0,
        tx,   ty,   tz,    1.0,
    ]
}

const VERTEX_SHADER: &str = r#"#version 330 core
layout(location = 0) in vec3 aPos;
layout(location = 1) in vec3 aNorm;
layout(location = 2) in float aMatId;

uniform mat4 uMvp;
uniform mat4 uModel;
uniform mat3 uNormalMat;

out vec3 vWorldPos;
out vec3 vNormal;
out float vMatId;

void main() {
    vec4 wp = uModel * vec4(aPos, 1.0);
    vWorldPos = wp.xyz;
    vNormal = normalize(uNormalMat * aNorm);
    vMatId = aMatId;
    gl_Position = uMvp * vec4(aPos, 1.0);
}
"#;

const FRAGMENT_SHADER: &str = r#"#version 330 core
in vec3 vWorldPos;
in vec3 vNormal;
in float vMatId;

uniform vec3 uShellColor;
uniform vec3 uCushionColor;
uniform vec3 uMetalColor;
uniform vec3 uAuraColor;
uniform vec3 uCamPos;
uniform float uDim;
uniform float uIsDark;

out vec4 fragColor;

const float PI = 3.14159265;

float distributionGGX(vec3 N, vec3 H, float roughness) {
    float a = roughness * roughness;
    float a2 = a * a;
    float NdotH = max(dot(N, H), 0.0);
    float denom = (NdotH * NdotH * (a2 - 1.0) + 1.0);
    return a2 / max(PI * denom * denom, 0.0001);
}

float geometrySchlickGGX(float NdotV, float roughness) {
    float r = (roughness + 1.0);
    float k = (r * r) / 8.0;
    return NdotV / max(NdotV * (1.0 - k) + k, 0.0001);
}

float geometrySmith(vec3 N, vec3 V, vec3 L, float roughness) {
    float ggx2 = geometrySchlickGGX(max(dot(N, V), 0.0), roughness);
    float ggx1 = geometrySchlickGGX(max(dot(N, L), 0.0), roughness);
    return ggx1 * ggx2;
}

vec3 fresnelSchlick(float cosTheta, vec3 F0) {
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

vec3 evalLight(vec3 N, vec3 V, vec3 L, vec3 radiance, vec3 albedo, float metallic, float roughness, vec3 F0) {
    vec3 H = normalize(V + L);
    float NdotL = max(dot(N, L), 0.0);
    float NdotV = max(dot(N, V), 0.001);
    float NDF = distributionGGX(N, H, roughness);
    float G = geometrySmith(N, V, L, roughness);
    vec3 F = fresnelSchlick(max(dot(H, V), 0.0), F0);

    vec3 spec = (NDF * G * F) / max(4.0 * NdotV * NdotL, 0.001);
    vec3 kD = (vec3(1.0) - F) * (1.0 - metallic);
    return (kD * albedo / PI + spec) * radiance * NdotL;
}

vec3 acesToneMap(vec3 x) {
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0);
}

void main() {
    vec3 N = normalize(vNormal);
    vec3 V = normalize(uCamPos - vWorldPos);

    vec3 albedo = uShellColor;
    float roughness = 0.44;
    float metallic = 0.04;

    if (vMatId > 2.5) {
        albedo = vec3(0.014, 0.015, 0.018);
        roughness = 0.88;
        metallic = 0.0;
    } else if (vMatId > 1.5) {
        albedo = uMetalColor;
        roughness = 0.24;
        metallic = 0.84;
    } else if (vMatId > 0.5) {
        albedo = uCushionColor;
        roughness = 0.56;
        metallic = 0.02;
    }

    vec3 F0 = mix(vec3(0.045), albedo, metallic);

    vec3 keyL = normalize(vec3(-0.62, 0.78, 0.72));
    vec3 keyRad = vec3(2.85, 2.75, 2.65) * mix(0.85, 1.0, uIsDark);

    vec3 fillL = normalize(vec3(0.85, 0.25, 0.55));
    vec3 fillRad = vec3(0.85, 0.96, 1.15);

    vec3 rimL = normalize(vec3(0.18, 0.55, -0.82));
    vec3 rimRad = vec3(1.65, 1.78, 2.05) * mix(0.65, 1.15, uIsDark);

    vec3 Lo = vec3(0.0);
    Lo += evalLight(N, V, keyL, keyRad, albedo, metallic, roughness, F0);
    Lo += evalLight(N, V, fillL, fillRad, albedo, metallic, roughness, F0);
    Lo += evalLight(N, V, rimL, rimRad, albedo, metallic, roughness, F0);

    float hemi = N.y * 0.5 + 0.5;
    vec3 skyAmb = mix(vec3(0.14, 0.16, 0.20), vec3(0.24, 0.26, 0.29), 1.0 - uIsDark);
    vec3 groundBounce = mix(vec3(0.04, 0.04, 0.05), uAuraColor * 0.36, clamp((uDim - 0.56) * 2.27, 0.0, 1.0));
    vec3 ambient = albedo * mix(groundBounce, skyAmb, hemi) * 0.72;

    float rimFresnel = pow(1.0 - max(dot(N, V), 0.0), 3.6);
    vec3 sheenColor = mix(vec3(0.55, 0.62, 0.72), mix(vec3(0.65, 0.72, 0.80), uAuraColor, 0.35), clamp((uDim - 0.56) * 2.27, 0.0, 1.0));
    vec3 rimGlow = sheenColor * rimFresnel * mix(0.20, 0.42, uIsDark);

    vec3 hdr = (ambient + Lo + rimGlow) * uDim;
    vec3 ldr = pow(acesToneMap(hdr), vec3(1.0 / 2.2));
    fragColor = vec4(ldr, 1.0);
}
"#;
