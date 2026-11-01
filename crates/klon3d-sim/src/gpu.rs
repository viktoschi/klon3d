//! Step 4b: the implicit step's CG solve on the GPU (`wgpu`, `f32`).
//!
//! **One kernel, one submission.** The whole PCG runs inside a single compute
//! shader: start values, `A·p`, the preconditioner, the reductions and the
//! updates, separated by `workgroupBarrier` inside one workgroup. A garment has
//! a few thousand points; a dispatch per phase would cost more than the work
//! (measured: 5 dispatches per iteration, 40 µs each, against a CPU iteration
//! of about the same length). One workgroup is 1024 lanes on one SM — slower
//! per lane than the full card, but without a host round trip and without
//! dispatch overhead between iterations.
//!
//! What moves: `A·p = M(1 + h·c)·p + h²·H·p` over distance edges, seams and
//! bending, gathered per point over a neighbour list; the block-Jacobi
//! preconditioner; the contact filter; the dot products as a fixed-order tree
//! reduction; the `Δv`/`r`/`z`/`p` updates.
//!
//! What stays on the CPU: forces, contact distance queries, the penetration
//! push, friction, self-collision and the release pass of held points.
//!
//! **No springs.** A step whose system carries springs (glue zones or
//! self-collision) is solved by the CPU backend and counted
//! ([`GpuLoeser::ausweichungen`]) instead of silently dropping them.
//!
//! **f32 on purpose** (ADR-018). How far `f32` is from the `f64` reference is
//! measured, not assumed: `crates/klon3d-sim/tests/gpu.rs` compares `A·p` and
//! the state after the same number of frames.
//!
//! **Iteration budget.** The CPU stops at `‖r‖ ≤ tol·‖b‖`; the GPU stops at the
//! same criterion, checked inside the shader between iterations, so the count
//! is the same as the CPU's.

use crate::implizit::{CgLauf, CpuLoeser, Fest, Loeser, System};

/// Offsets of the vector segments inside the data buffer, in `vec4` units.
#[derive(Debug, Clone, Copy)]
struct Lage {
    dv: u32,
    r: u32,
    z: u32,
    dir: u32,
    ap: u32,
    p: u32,
    b: u32,
    v: u32,
    fb: u32,
    zfb: u32,
    sk: u32,
    ende: u32,
}

impl Lage {
    fn neu(n: usize) -> Lage {
        let n32 = n as u32;
        let mut o = 0;
        let mut naechste = |len: u32| {
            let hier = o;
            o += len;
            hier
        };
        Lage {
            dv: naechste(n32),
            r: naechste(n32),
            z: naechste(n32),
            dir: naechste(n32),
            ap: naechste(n32),
            p: naechste(n32),
            b: naechste(n32),
            v: naechste(n32),
            fb: naechste(n32),
            zfb: naechste(n32),
            sk: naechste(8),
            ende: o,
        }
    }
}

/// The uniform block; mirrors `Params` in the shader.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    n: u32,
    kanten: u32,
    biege: u32,
    modus: u32,
    cg_max: u32,
    l_dv: u32,
    l_r: u32,
    l_z: u32,
    l_dir: u32,
    l_ap: u32,
    l_b: u32,
    l_v: u32,
    l_fb: u32,
    l_zfb: u32,
    l_sk: u32,
    l_p: u32,
    h2: f32,
    tol: f32,
    boden: f32,
    _p: f32,
}

const WGSL: &str = r#"
struct Params {
    n: u32,
    kanten: u32,
    biege: u32,
    modus: u32,
    cg_max: u32,
    l_dv: u32,
    l_r: u32,
    l_z: u32,
    l_dir: u32,
    l_ap: u32,
    l_b: u32,
    l_v: u32,
    l_fb: u32,
    l_zfb: u32,
    l_sk: u32,
    l_p: u32,
    h2: f32,
    tol: f32,
    boden: f32,
    _p: f32,
};

@group(0) @binding(0) var<uniform> par: Params;
// mass·(1 + h·c), inverse mass, held flag, required normal velocity
@group(0) @binding(1) var<storage, read_write> kopf: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> normale: array<vec4<f32>>;
// inverse of the 3x3 diagonal block, three rows per point
@group(0) @binding(3) var<storage, read_write> pinv: array<vec4<f32>>;
// dv, r, z, dir, ap, b, v, fb, zfb, then eight scalars
@group(0) @binding(4) var<storage, read_write> daten: array<vec4<f32>>;
// n+1 neighbour-list starts, then four u32 per entry: kind, index, slot, 0
@group(0) @binding(5) var<storage, read_write> liste: array<u32>;
// two vec4 per distance constraint: (i, j, k, cross), (n, 0)
@group(0) @binding(6) var<storage, read_write> kante: array<vec4<f32>>;
// six vec4 per bending constraint: (i0, i1, i2, i3), (k, 0, 0, 0), g0..g3
@group(0) @binding(7) var<storage, read_write> biege: array<vec4<f32>>;

var<workgroup> red_a: array<f32, 1024>;
var<workgroup> red_b: array<f32, 1024>;
var<workgroup> sk: array<f32, 8>;

fn lies(off: u32, i: u32) -> vec3<f32> {
    return daten[off + i].xyz;
}

fn schreib(off: u32, i: u32, v: vec3<f32>) {
    daten[off + i] = vec4<f32>(v, 0.0);
}

fn anf(i: u32) -> u32 {
    return liste[i];
}

fn eintrag(i: u32, feld: u32) -> u32 {
    return liste[par.n + 1u + 4u * i + feld];
}

fn filtere(i: u32, v: vec3<f32>) -> vec3<f32> {
    let k = kopf[i];
    if (k.y <= 0.0) {
        return vec3<f32>(0.0);
    }
    if (k.z > 0.5) {
        let nl = normale[i].xyz;
        return v - nl * dot(v, nl);
    }
    return v;
}

// ap = M(1 + h c)·p + h^2·H·p gathered over the point's entries.
// `gefiltert`: the contact filter is applied (what the CG iterates on).
fn mal_a(tid: u32, off: u32, gefiltert: bool) {
    for (var i = tid; i < par.n; i = i + 1024u) {
        let pi = lies(off, i);
        var sum = vec3<f32>(0.0);
        let von = anf(i);
        let bis = anf(i + 1u);
        for (var e = von; e < bis; e = e + 1u) {
            let art = eintrag(e, 0u);
            let c = eintrag(e, 1u);
            if (art == 0u) {
                let a = kante[2u * c];
                let b = kante[2u * c + 1u];
                let ist_i = u32(a.x) == i;
                let j = select(u32(a.x), u32(a.y), ist_i);
                // d ist schon p_selbst - p_partner; der Block ist auf beiden
                // Seiten derselbe, ein Vorzeichenwechsel wäre hier falsch.
                let d = pi - lies(off, j);
                let nr = b.xyz;
                sum = sum + a.z * (nr * dot(nr, d) * (1.0 - a.w) + d * a.w);
            } else {
                let q = biege[6u * c];
                let idx = vec4<u32>(u32(q.x), u32(q.y), u32(q.z), u32(q.w));
                var gp = dot(biege[6u * c + 2u].xyz, lies(off, idx.x));
                gp = gp + dot(biege[6u * c + 3u].xyz, lies(off, idx.y));
                gp = gp + dot(biege[6u * c + 4u].xyz, lies(off, idx.z));
                gp = gp + dot(biege[6u * c + 5u].xyz, lies(off, idx.w));
                let m = eintrag(e, 2u);
                sum = sum + biege[6u * c + 1u].x * biege[6u * c + 2u + m].xyz * gp;
            }
        }
        var aus = sum * par.h2 + kopf[i].x * pi;
        if (gefiltert) {
            aus = filtere(i, aus);
        }
        schreib(par.l_ap, i, aus);
    }
}

// z = P^-1·v, written to `off`.
fn vorkond(tid: u32, von: u32, nach: u32) {
    for (var i = tid; i < par.n; i = i + 1024u) {
        let v = lies(von, i);
        let p0 = pinv[3u * i];
        let p1 = pinv[3u * i + 1u];
        let p2 = pinv[3u * i + 2u];
        schreib(nach, i, vec3<f32>(dot(p0.xyz, v), dot(p1.xyz, v), dot(p2.xyz, v)));
    }
}

fn summe(v: f32, tid: u32, zweite: bool) -> f32 {
    if (zweite) {
        red_b[tid] = v;
    } else {
        red_a[tid] = v;
    }
    workgroupBarrier();
    var s = 512u;
    loop {
        if (s == 0u) {
            break;
        }
        if (tid < s) {
            if (zweite) {
                red_b[tid] = red_b[tid] + red_b[tid + s];
            } else {
                red_a[tid] = red_a[tid] + red_a[tid + s];
            }
        }
        workgroupBarrier();
        s = s / 2u;
    }
    if (zweite) {
        return red_b[0];
    }
    return red_a[0];
}

@compute @workgroup_size(1024)
fn rechnen(@builtin(local_invocation_index) tid: u32) {
    if (par.modus == 1u) {
        // Nur A·p, für den Vergleich der Rechenwege.
        mal_a(tid, par.l_p, false);
        return;
    }
    let n = par.n;
    // Warmstart und gefilterte rechte Seite.
    for (var i = tid; i < n; i = i + 1024u) {
        let fb = filtere(i, lies(par.l_b, i));
        schreib(par.l_fb, i, fb);
        let k = kopf[i];
        if (k.y <= 0.0) {
            schreib(par.l_dv, i, vec3<f32>(0.0));
            continue;
        }
        var dv = filtere(i, lies(par.l_dv, i));
        if (k.z > 0.5) {
            let nl = normale[i].xyz;
            dv = dv + nl * (k.w - dot(lies(par.l_v, i), nl));
        }
        schreib(par.l_dv, i, dv);
    }
    workgroupBarrier();
    mal_a(tid, par.l_dv, false);
    workgroupBarrier();
    // r = filter(b - A dv), z = P^-1 r, dir = filter(z), zfb = P^-1 fb.
    for (var i = tid; i < n; i = i + 1024u) {
        let r = filtere(i, lies(par.l_b, i) - lies(par.l_ap, i));
        schreib(par.l_r, i, select(vec3<f32>(0.0), r, kopf[i].y > 0.0));
    }
    workgroupBarrier();
    vorkond(tid, par.l_r, par.l_z);
    vorkond(tid, par.l_fb, par.l_zfb);
    workgroupBarrier();
    for (var i = tid; i < n; i = i + 1024u) {
        schreib(par.l_dir, i, filtere(i, lies(par.l_z, i)));
    }
    workgroupBarrier();
    var a = 0.0;
    var bsum = 0.0;
    for (var i = tid; i < n; i = i + 1024u) {
        if (kopf[i].y > 0.0) {
            a = a + dot(lies(par.l_r, i), lies(par.l_dir, i));
            bsum = bsum + dot(lies(par.l_fb, i), lies(par.l_zfb, i));
        }
    }
    let delta0 = summe(a, tid, false);
    let bb = summe(bsum, tid, true);
    if (tid == 0u) {
        sk[0] = delta0;
        sk[3] = bb;
        let ziel = max(par.tol * par.tol * max(bb, delta0), par.boden);
        sk[6] = ziel;
        sk[4] = 0.0;
        sk[5] = 0.0;
    }
    workgroupBarrier();
    var it = 0u;
    loop {
        if (it >= par.cg_max) {
            break;
        }
        if (sk[0] <= sk[6]) {
            break;
        }
        mal_a(tid, par.l_dir, true);
        workgroupBarrier();
        var ca = 0.0;
        for (var i = tid; i < n; i = i + 1024u) {
            if (kopf[i].y > 0.0) {
                ca = ca + dot(lies(par.l_dir, i), lies(par.l_ap, i));
            }
        }
        let ca_summe = summe(ca, tid, false);
        if (tid == 0u) {
            if (ca_summe > 1e-30) {
                sk[4] = sk[0] / ca_summe;
            } else {
                sk[4] = 0.0;
            }
        }
        workgroupBarrier();
        let alpha = sk[4];
        for (var i = tid; i < n; i = i + 1024u) {
            schreib(par.l_dv, i, lies(par.l_dv, i) + alpha * lies(par.l_dir, i));
            schreib(par.l_r, i, lies(par.l_r, i) - alpha * lies(par.l_ap, i));
        }
        workgroupBarrier();
        vorkond(tid, par.l_r, par.l_z);
        workgroupBarrier();
        var neu = 0.0;
        for (var i = tid; i < n; i = i + 1024u) {
            if (kopf[i].y > 0.0) {
                neu = neu + dot(lies(par.l_r, i), lies(par.l_z, i));
            }
        }
        let neu_summe = summe(neu, tid, false);
        if (tid == 0u) {
            let alt = sk[0];
            var beta = 0.0;
            if (alt > 1e-30) {
                beta = neu_summe / alt;
            }
            sk[0] = neu_summe;
            sk[5] = beta;
        }
        workgroupBarrier();
        let beta = sk[5];
        for (var i = tid; i < n; i = i + 1024u) {
            schreib(par.l_dir, i, filtere(i, lies(par.l_z, i) + beta * lies(par.l_dir, i)));
        }
        workgroupBarrier();
        it = it + 1u;
    }
    if (tid == 0u) {
        daten[par.l_sk] = vec4<f32>(sk[0], 0.0, 0.0, 0.0);
        daten[par.l_sk + 1u] = vec4<f32>(sk[3], 0.0, 0.0, 0.0);
        daten[par.l_sk + 2u] = vec4<f32>(f32(it), 0.0, 0.0, 0.0);
        daten[par.l_sk + 3u] = vec4<f32>(sk[6], 0.0, 0.0, 0.0);
    }
}
"#;

/// The buffers of one scene size.
#[derive(Clone)]
struct Speicher {
    n: usize,
    kanten: usize,
    biege: usize,
    kopf: wgpu::Buffer,
    normale: wgpu::Buffer,
    pinv: wgpu::Buffer,
    daten: wgpu::Buffer,
    kante: wgpu::Buffer,
    biege_b: wgpu::Buffer,
    params: wgpu::Buffer,
    bind: wgpu::BindGroup,
    lesen: wgpu::Buffer,
}

/// Step 4b: the `wgpu` backend of [`Loeser`].
///
/// [`GpuLoeser::neu`] returns `Err` on a machine without an adapter; the caller
/// then keeps the CPU backend, and the tests skip instead of failing.
pub struct GpuLoeser {
    geraet: String,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    lage: wgpu::BindGroupLayout,
    speicher: Option<Speicher>,
    cpu: CpuLoeser,
    ausweich: usize,
}

impl std::fmt::Debug for GpuLoeser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GpuLoeser({})", self.geraet)
    }
}

impl GpuLoeser {
    /// Opens the GPU. `Err` carries the reason (no adapter, no device).
    pub fn neu() -> Result<Self, String> {
        let inst = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(inst.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .map_err(|_| "kein GPU-Adapter".to_string())?;
        let info = adapter.get_info();
        let geraet = format!("{} ({:?}, {:?})", info.name, info.backend, info.device_type);
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("klon3d CG"),
            // Der Löser ist ein einziger Workgroup-Kernel; die Vorgabe von
            // WebGPU sind 256 Aufrufe je Workgroup, hier sind es 1024.
            required_limits: wgpu::Limits {
                max_compute_invocations_per_workgroup: 1024,
                max_compute_workgroup_size_x: 1024,
                ..Default::default()
            },
            ..Default::default()
        }))
        .map_err(|e| format!("GPU-Gerät: {e}"))?;

        let modul = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("klon3d CG"),
            source: wgpu::ShaderSource::Wgsl(WGSL.into()),
        });
        let eintrag = |binding: u32, ty: wgpu::BufferBindingType| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let lage = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("klon3d CG"),
            entries: &[
                eintrag(0, wgpu::BufferBindingType::Uniform),
                eintrag(1, wgpu::BufferBindingType::Storage { read_only: false }),
                eintrag(2, wgpu::BufferBindingType::Storage { read_only: false }),
                eintrag(3, wgpu::BufferBindingType::Storage { read_only: false }),
                eintrag(4, wgpu::BufferBindingType::Storage { read_only: false }),
                eintrag(5, wgpu::BufferBindingType::Storage { read_only: false }),
                eintrag(6, wgpu::BufferBindingType::Storage { read_only: false }),
                eintrag(7, wgpu::BufferBindingType::Storage { read_only: false }),
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("klon3d CG"),
            bind_group_layouts: &[Some(&lage)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("klon3d CG"),
            layout: Some(&pl),
            module: &modul,
            entry_point: Some("rechnen"),
            compilation_options: Default::default(),
            cache: None,
        });
        Ok(GpuLoeser {
            geraet,
            device,
            queue,
            pipeline,
            lage,
            speicher: None,
            cpu: CpuLoeser,
            ausweich: 0,
        })
    }

    /// Backend, device name and device type, for the report.
    pub fn geraet(&self) -> &str {
        &self.geraet
    }

    /// Steps that went to the CPU because the system carried springs.
    pub fn ausweichungen(&self) -> usize {
        self.ausweich
    }

    fn puffer(&self, bytes: u64, lesen: bool) -> wgpu::Buffer {
        let usage = if lesen {
            wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST
        } else {
            wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::UNIFORM
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC
        };
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: bytes.max(16),
            usage,
            mapped_at_creation: false,
        })
    }

    /// The gathering structure: per point its two distance constraints and four
    /// bending slots, with the slot the point occupies in the bending
    /// constraint. Depends on the topology only, so it is written once.
    fn liste_bauen(sys: &System<'_>) -> Vec<u32> {
        let n = sys.masse.len();
        let mut zahl = vec![0u32; n];
        for k in sys.kanten.iter().chain(&sys.naehte) {
            zahl[k.i as usize] += 1;
            zahl[k.j as usize] += 1;
        }
        for b in &sys.biegungen {
            // One entry per point and constraint, carrying the slot that point
            // occupies in the constraint.
            for q in b.punkte {
                zahl[q as usize] += 1;
            }
        }
        let mut aus = vec![0u32; n + 1];
        for i in 0..n {
            aus[i + 1] = aus[i] + zahl[i];
        }
        let mut fuell = aus.clone();
        let mut eintraege = vec![0u32; aus[n] as usize * 4];
        let eintrag =
            |i: usize, art: u32, c: u32, m: u32, fuell: &mut Vec<u32>, eintraege: &mut Vec<u32>| {
                let e = fuell[i] as usize;
                fuell[i] += 1;
                eintraege[4 * e] = art;
                eintraege[4 * e + 1] = c;
                eintraege[4 * e + 2] = m;
                eintraege[4 * e + 3] = 0;
            };
        // Kanten und Nähte teilen sich einen Indexraum: erst Kanten, dann Nähte.
        let naht_start = sys.kanten.len() as u32;
        for (c, k) in sys.kanten.iter().enumerate() {
            eintrag(k.i as usize, 0, c as u32, 0, &mut fuell, &mut eintraege);
            eintrag(k.j as usize, 0, c as u32, 0, &mut fuell, &mut eintraege);
        }
        for (c, k) in sys.naehte.iter().enumerate() {
            eintrag(
                k.i as usize,
                0,
                naht_start + c as u32,
                0,
                &mut fuell,
                &mut eintraege,
            );
            eintrag(
                k.j as usize,
                0,
                naht_start + c as u32,
                0,
                &mut fuell,
                &mut eintraege,
            );
        }
        for (c, b) in sys.biegungen.iter().enumerate() {
            for m in 0..4 {
                eintrag(
                    b.punkte[m] as usize,
                    1,
                    c as u32,
                    m as u32,
                    &mut fuell,
                    &mut eintraege,
                );
            }
        }
        aus.extend_from_slice(&eintraege);
        aus
    }

    fn hochladen(&mut self, sys: &System<'_>, fest: &[Option<Fest>], dv: &[[f64; 3]]) -> Speicher {
        let n = sys.masse.len();
        let kanten = sys.kanten.len() + sys.naehte.len();
        let neu = match &self.speicher {
            Some(s) => s.n != n || s.kanten != kanten || s.biege != sys.biegungen.len(),
            None => true,
        };
        if neu {
            let lage = Lage::neu(n);
            let liste = Self::liste_bauen(sys);
            let kopf = self.puffer(n as u64 * 16, false);
            let normale = self.puffer(n as u64 * 16, false);
            let pinv = self.puffer(n as u64 * 48, false);
            let daten = self.puffer(lage.ende as u64 * 16, false);
            let liste_b = self.puffer(liste.len() as u64 * 4, false);
            let kante = self.puffer((kanten * 2).max(1) as u64 * 16, false);
            let biege_b = self.puffer((sys.biegungen.len() * 6).max(1) as u64 * 16, false);
            let params = self.puffer(std::mem::size_of::<Params>() as u64, false);
            let lesen = self.puffer((n * 16 + 8 * 16) as u64, true);
            self.queue
                .write_buffer(&liste_b, 0, bytemuck::cast_slice(&liste));
            let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.lage,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: params.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: kopf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: normale.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: pinv.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: daten.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: liste_b.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: kante.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: biege_b.as_entire_binding(),
                    },
                ],
            });
            self.speicher = Some(Speicher {
                n,
                kanten,
                biege: sys.biegungen.len(),
                kopf,
                normale,
                pinv,
                daten,
                kante,
                biege_b,
                params,
                bind,
                lesen,
            });
        }
        let s = self.speicher.clone().expect("eben gebaut");
        let lage = Lage::neu(n);
        // Constraints: sie hängen an der aktuellen Lage (Richtung, Querfaktor,
        // Diederwinkel-Gradient) und gehen in jedem Unterschritt neu hoch.
        let mut kanten_f = Vec::with_capacity(kanten * 8);
        for k in sys.kanten.iter().chain(&sys.naehte) {
            kanten_f.extend_from_slice(&[k.i as f32, k.j as f32, k.k as f32, k.quer as f32]);
            kanten_f.extend_from_slice(&[
                k.richtung[0] as f32,
                k.richtung[1] as f32,
                k.richtung[2] as f32,
                0.0,
            ]);
        }
        self.queue
            .write_buffer(&s.kante, 0, bytemuck::cast_slice(&kanten_f));
        let mut biege_f = Vec::with_capacity(sys.biegungen.len() * 24);
        for b in &sys.biegungen {
            biege_f.extend_from_slice(&[
                b.punkte[0] as f32,
                b.punkte[1] as f32,
                b.punkte[2] as f32,
                b.punkte[3] as f32,
            ]);
            biege_f.extend_from_slice(&[b.k as f32, 0.0, 0.0, 0.0]);
            for m in 0..4 {
                biege_f.extend_from_slice(&[
                    b.grad[m][0] as f32,
                    b.grad[m][1] as f32,
                    b.grad[m][2] as f32,
                    0.0,
                ]);
            }
        }
        self.queue
            .write_buffer(&s.biege_b, 0, bytemuck::cast_slice(&biege_f));
        // Punkte.
        let mut kopf = Vec::with_capacity(n * 4);
        let mut normale = Vec::with_capacity(n * 4);
        let mut pinv = Vec::with_capacity(n * 12);
        let (mut dv_f, mut b_f, mut v_f) = (
            Vec::with_capacity(n * 4),
            Vec::with_capacity(n * 4),
            Vec::with_capacity(n * 4),
        );
        for i in 0..n {
            kopf.extend_from_slice(&[
                (sys.masse[i] * (1.0 + sys.h * sys.c)) as f32,
                sys.w[i] as f32,
                if fest[i].is_some() { 1.0 } else { 0.0 },
                fest[i].map(|f| f.vn_soll).unwrap_or(0.0) as f32,
            ]);
            let nrm = fest[i].map(|f| f.normale).unwrap_or([0.0; 3]);
            normale.extend_from_slice(&[nrm[0] as f32, nrm[1] as f32, nrm[2] as f32, 0.0]);
            for a in 0..3 {
                pinv.extend_from_slice(&[
                    sys.pinv[i][a][0] as f32,
                    sys.pinv[i][a][1] as f32,
                    sys.pinv[i][a][2] as f32,
                    0.0,
                ]);
            }
            dv_f.extend_from_slice(&[dv[i][0] as f32, dv[i][1] as f32, dv[i][2] as f32, 0.0]);
            b_f.extend_from_slice(&[
                sys.rhs[i][0] as f32,
                sys.rhs[i][1] as f32,
                sys.rhs[i][2] as f32,
                0.0,
            ]);
            v_f.extend_from_slice(&[
                sys.v[i][0] as f32,
                sys.v[i][1] as f32,
                sys.v[i][2] as f32,
                0.0,
            ]);
        }
        self.queue
            .write_buffer(&s.kopf, 0, bytemuck::cast_slice(&kopf));
        self.queue
            .write_buffer(&s.normale, 0, bytemuck::cast_slice(&normale));
        self.queue
            .write_buffer(&s.pinv, 0, bytemuck::cast_slice(&pinv));
        self.queue
            .write_buffer(&s.daten, lage.dv as u64 * 16, bytemuck::cast_slice(&dv_f));
        self.queue
            .write_buffer(&s.daten, lage.b as u64 * 16, bytemuck::cast_slice(&b_f));
        self.queue
            .write_buffer(&s.daten, lage.v as u64 * 16, bytemuck::cast_slice(&v_f));
        s
    }

    fn params(&self, s: &Speicher, sys: &System<'_>, modus: u32) -> Params {
        let lage = Lage::neu(s.n);
        Params {
            n: s.n as u32,
            kanten: s.kanten as u32,
            biege: s.biege as u32,
            modus,
            cg_max: sys.cg_max as u32,
            l_dv: lage.dv,
            l_r: lage.r,
            l_z: lage.z,
            l_dir: lage.dir,
            l_ap: lage.ap,
            l_b: lage.b,
            l_v: lage.v,
            l_fb: lage.fb,
            l_zfb: lage.zfb,
            l_sk: lage.sk,
            l_p: lage.p,
            h2: (sys.h * sys.h) as f32,
            tol: sys.cg_toleranz as f32,
            boden: (sys.masse.iter().sum::<f64>() * 1e-18) as f32,
            _p: 0.0,
        }
    }

    /// Copies `vec4` vectors from `lage` and appends the eight scalars, then
    /// maps once: two readbacks would be two full syncs.
    fn lesen_mit_skalaren(&self, s: &Speicher, lage: u32, vec4: usize) -> (Vec<f32>, [f32; 8]) {
        let sk = Lage::neu(s.n).sk;
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        enc.copy_buffer_to_buffer(&s.daten, lage as u64 * 16, &s.lesen, 0, vec4 as u64 * 16);
        enc.copy_buffer_to_buffer(&s.daten, sk as u64 * 16, &s.lesen, vec4 as u64 * 16, 8 * 16);
        self.queue.submit([enc.finish()]);
        let slice = s.lesen.slice(..(vec4 as u64 + 8) * 16);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .ok();
        let daten = slice.get_mapped_range().unwrap();
        let f: Vec<f32> = bytemuck::cast_slice(&daten).to_vec();
        drop(daten);
        s.lesen.unmap();
        let mut skalar = [0.0f32; 8];
        for (i, a) in skalar.iter_mut().enumerate() {
            *a = f[vec4 * 4 + i * 4];
        }
        (f[..vec4 * 4].to_vec(), skalar)
    }

    fn lesen(&self, s: &Speicher, lage: u32, vec4: usize) -> Vec<f32> {
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        enc.copy_buffer_to_buffer(&s.daten, lage as u64 * 16, &s.lesen, 0, vec4 as u64 * 16);
        self.queue.submit([enc.finish()]);
        let slice = s.lesen.slice(..vec4 as u64 * 16);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .ok();
        let daten = slice.get_mapped_range().unwrap();
        let f: Vec<f32> = bytemuck::cast_slice(&daten).to_vec();
        drop(daten);
        s.lesen.unmap();
        f
    }

    fn loese_gpu(
        &mut self,
        sys: &System<'_>,
        fest: &[Option<Fest>],
        dv: &mut [[f64; 3]],
    ) -> CgLauf {
        let n = sys.masse.len();
        let s = self.hochladen(sys, fest, dv);
        let par = self.params(&s, sys, 0);
        self.queue
            .write_buffer(&s.params, 0, bytemuck::bytes_of(&par));
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            cp.set_pipeline(&self.pipeline);
            cp.set_bind_group(0, &s.bind, &[]);
            cp.dispatch_workgroups(1, 1, 1);
        }
        self.queue.submit([enc.finish()]);
        let lage = Lage::neu(n);
        let (dv_f, sk) = self.lesen_mit_skalaren(&s, lage.dv, n);
        for i in 0..n {
            dv[i] = [
                dv_f[i * 4] as f64,
                dv_f[i * 4 + 1] as f64,
                dv_f[i * 4 + 2] as f64,
            ];
        }
        // Je Skalar ein vec4: erstes Feld = Wert.
        let (delta, bb, it, ziel) = (sk[0] as f64, sk[1] as f64, sk[2] as f64, sk[3] as f64);
        let boden = sys.masse.iter().sum::<f64>() * 1e-18;
        CgLauf {
            iterationen: it as usize,
            nicht_konvergiert: delta > ziel,
            rest: (delta / bb.max(boden)).sqrt(),
        }
    }

    /// `A·p` on the device, for the acceptance test.
    pub fn mal_a_roh(&mut self, sys: &System<'_>, p: &[[f64; 3]]) -> Vec<[f64; 3]> {
        let n = sys.masse.len();
        let s = self.hochladen(sys, &vec![None; n], &vec![[0.0; 3]; n]);
        let lage = Lage::neu(n);
        let mut p_f = Vec::with_capacity(n * 4);
        for q in p {
            p_f.extend_from_slice(&[q[0] as f32, q[1] as f32, q[2] as f32, 0.0]);
        }
        self.queue
            .write_buffer(&s.daten, lage.p as u64 * 16, bytemuck::cast_slice(&p_f));
        let par = self.params(&s, sys, 1);
        self.queue
            .write_buffer(&s.params, 0, bytemuck::bytes_of(&par));
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            cp.set_pipeline(&self.pipeline);
            cp.set_bind_group(0, &s.bind, &[]);
            cp.dispatch_workgroups(1, 1, 1);
        }
        self.queue.submit([enc.finish()]);
        let f = self.lesen(&s, lage.ap, n);
        (0..n)
            .map(|i| [f[i * 4] as f64, f[i * 4 + 1] as f64, f[i * 4 + 2] as f64])
            .collect()
    }
}

impl Loeser for GpuLoeser {
    fn name(&self) -> &'static str {
        "GPU (f32, wgpu)"
    }

    fn loese(&mut self, sys: &System<'_>, fest: &[Option<Fest>], dv: &mut [[f64; 3]]) -> CgLauf {
        if !sys.federn.is_empty() {
            self.ausweich += 1;
            return self.cpu.loese(sys, fest, dv);
        }
        self.loese_gpu(sys, fest, dv)
    }

    fn mal_a(&mut self, sys: &System<'_>, p: &[[f64; 3]], out: &mut [[f64; 3]]) {
        let gpu = self.mal_a_roh(sys, p);
        for (o, g) in out.iter_mut().zip(&gpu) {
            *o = *g;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn einstiegspunkt_im_shader() {
        assert!(super::WGSL.contains("fn rechnen("));
        assert!(super::WGSL.contains("fn mal_a("));
    }
}
