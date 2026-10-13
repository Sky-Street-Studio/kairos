use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Range;

use bytemuck::{Pod, Zeroable};
use glyphon::{Cache, Resolution, SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport};

use crate::layout::{Letterbox, Rect};
use crate::text::{Fonts, glyph_color};
use crate::{Color, Draw, Frame, TextAlign, TextSpec, TransitionKind};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
    rect: [f32; 4],
    radius: f32,
}

#[derive(Debug, Clone, PartialEq)]
enum Source {
    White,
    Image(String),
    Scene,
    Previous,
}

struct Batch {
    source: Source,
    range: Range<u32>,
}

struct Image {
    bind_group: wgpu::BindGroup,
    size: (u32, u32),
}

struct Target {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
}

struct Transition {
    kind: TransitionKind,
    elapsed: f32,
    duration: f32,
}

struct Globals {
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

/// Draws [`Frame`]s into a surface texture, letterboxing the virtual screen.
pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    pipeline: wgpu::RenderPipeline,
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    scene_globals: Globals,
    surface_globals: Globals,
    white: wgpu::BindGroup,
    images: HashMap<String, Image>,
    scene: Target,
    previous: Target,
    transition: Option<Transition>,
    virtual_size: (f32, f32),
    surface_size: (u32, u32),
    letterbox: Letterbox,
    vertex_buffer: wgpu::Buffer,
    swash: SwashCache,
    atlas: TextAtlas,
    scene_viewport: Viewport,
    surface_viewport: Viewport,
    text_renderers: Vec<TextRenderer>,
    buffers: HashMap<u64, glyphon::Buffer>,
}

const VERTEX_SIZE: u64 = std::mem::size_of::<Vertex>() as u64;

impl Renderer {
    /// `format` is the format of the views passed to [`Renderer::render`]; use an sRGB format.
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        virtual_size: (u32, u32),
        surface_size: (u32, u32),
    ) -> Renderer {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("kairos quads"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
        });
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kairos globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kairos texture"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("kairos quads"),
            bind_group_layouts: &[Some(&globals_layout), Some(&texture_layout)],
            ..Default::default()
        });
        let attributes =
            wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4, 3 => Float32x4, 4 => Float32];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("kairos quads"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: VERTEX_SIZE,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            cache: None,
            multiview_mask: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("kairos linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let globals = |label| {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: 16,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &globals_layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }],
            });
            Globals { buffer, bind_group }
        };
        let scene_globals = globals("kairos scene globals");
        let surface_globals = globals("kairos surface globals");
        let vertex_buffer = vertex_buffer(&device, 1024);

        let cache = Cache::new(&device);
        let atlas = TextAtlas::new(&device, &queue, &cache, format);
        let scene_viewport = Viewport::new(&device, &cache);
        let surface_viewport = Viewport::new(&device, &cache);

        let white_texture = upload(&device, &queue, (1, 1), &[255; 4]);
        let white = texture_bind_group(&device, &texture_layout, &sampler, &white_texture);
        let virtual_size = (virtual_size.0 as f32, virtual_size.1 as f32);
        let letterbox = Letterbox::fit(virtual_size, (surface_size.0 as f32, surface_size.1 as f32));
        let target_size = scene_size(virtual_size, letterbox);
        let scene = target(&device, &texture_layout, &sampler, format, target_size);
        let previous = target(&device, &texture_layout, &sampler, format, target_size);

        let mut renderer = Renderer {
            device,
            queue,
            format,
            pipeline,
            texture_layout,
            sampler,
            scene_globals,
            surface_globals,
            white,
            images: HashMap::new(),
            scene,
            previous,
            transition: None,
            virtual_size,
            surface_size,
            letterbox,
            vertex_buffer,
            swash: SwashCache::new(),
            atlas,
            scene_viewport,
            surface_viewport,
            text_renderers: Vec::new(),
            buffers: HashMap::new(),
        };
        renderer.resize(surface_size);
        renderer
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn letterbox(&self) -> Letterbox {
        self.letterbox
    }

    pub fn virtual_size(&self) -> (f32, f32) {
        self.virtual_size
    }

    pub fn max_texture_size(&self) -> u32 {
        self.device.limits().max_texture_dimension_2d
    }

    pub fn resize(&mut self, surface_size: (u32, u32)) {
        let surface_size = (surface_size.0.max(1), surface_size.1.max(1));
        self.surface_size = surface_size;
        self.letterbox = Letterbox::fit(self.virtual_size, (surface_size.0 as f32, surface_size.1 as f32));
        let size = scene_size(self.virtual_size, self.letterbox);
        if self.scene.texture.width() != size.0 || self.scene.texture.height() != size.1 {
            self.scene = target(&self.device, &self.texture_layout, &self.sampler, self.format, size);
            self.previous = target(&self.device, &self.texture_layout, &self.sampler, self.format, size);
        }
        let globals = |w: u32, h: u32| bytemuck::cast_slice(&[w as f32, h as f32, 0.0, 0.0]).to_vec();
        self.queue.write_buffer(&self.scene_globals.buffer, 0, &globals(size.0, size.1));
        self.queue.write_buffer(&self.surface_globals.buffer, 0, &globals(surface_size.0, surface_size.1));
    }

    /// Uploads straight-alpha RGBA8 pixels under `name`, replacing any image with that name.
    pub fn upload_image(&mut self, name: &str, size: (u32, u32), rgba: &[u8]) {
        let texture = upload(&self.device, &self.queue, size, rgba);
        let bind_group = texture_bind_group(&self.device, &self.texture_layout, &self.sampler, &texture);
        self.images.insert(name.to_string(), Image { bind_group, size });
    }

    pub fn image_size(&self, name: &str) -> Option<(u32, u32)> {
        self.images.get(name).map(|i| i.size)
    }

    pub fn remove_image(&mut self, name: &str) {
        self.images.remove(name);
    }

    /// Snapshots the last rendered scene; the next frames blend from it to the new scene.
    pub fn begin_transition(&mut self, kind: TransitionKind, seconds: f32) {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_texture(
            self.scene.texture.as_image_copy(),
            self.previous.texture.as_image_copy(),
            self.scene.texture.size(),
        );
        self.queue.submit(Some(encoder.finish()));
        self.transition = Some(Transition { kind, elapsed: 0.0, duration: seconds.max(0.01) });
    }

    pub fn finish_transition(&mut self) {
        self.transition = None;
    }

    pub fn is_transitioning(&self) -> bool {
        self.transition.is_some()
    }

    pub fn tick(&mut self, dt: f32) {
        if let Some(t) = &mut self.transition {
            t.elapsed += dt;
            if t.elapsed >= t.duration {
                self.transition = None;
            }
        }
    }

    pub fn render(&mut self, fonts: &mut Fonts, frame: &Frame, view: &wgpu::TextureView) {
        let lb = self.letterbox;
        let mut vertices = Vec::new();
        let scene_batches = self.push_layer(&mut vertices, &frame.scene, Letterbox { x: 0.0, y: 0.0, ..lb });
        let composite = self.push_composite(&mut vertices);
        let ui_batches: Vec<Vec<Batch>> = frame.ui.iter().map(|l| self.push_layer(&mut vertices, l, lb)).collect();

        let bytes = (vertices.len() as u64 * VERTEX_SIZE).max(VERTEX_SIZE);
        if self.vertex_buffer.size() < bytes {
            self.vertex_buffer = vertex_buffer(&self.device, bytes.next_power_of_two());
        }
        self.queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));

        while self.text_renderers.len() < 1 + frame.ui.len() {
            let renderer = TextRenderer::new(&mut self.atlas, &self.device, wgpu::MultisampleState::default(), None);
            self.text_renderers.push(renderer);
        }
        let scene_size = (self.scene.texture.width(), self.scene.texture.height());
        self.scene_viewport.update(&self.queue, Resolution { width: scene_size.0, height: scene_size.1 });
        let surface = Resolution { width: self.surface_size.0, height: self.surface_size.1 };
        self.surface_viewport.update(&self.queue, surface);

        let mut used = HashSet::new();
        let layers: Vec<(&[Draw], Letterbox)> = std::iter::once((&frame.scene[..], Letterbox { x: 0.0, y: 0.0, ..lb }))
            .chain(frame.ui.iter().map(|l| (&l[..], lb)))
            .collect();
        let keys: Vec<Vec<(u64, &TextSpec)>> = layers
            .iter()
            .map(|(layer, lb)| {
                texts(layer)
                    .map(|spec| {
                        let key = buffer_key(spec, lb.scale);
                        used.insert(key);
                        self.buffers.entry(key).or_insert_with(|| fonts.layout(spec, Some(spec.rect.w), lb.scale));
                        (key, spec)
                    })
                    .collect()
            })
            .collect();
        self.buffers.retain(|k, _| used.contains(k));

        for (i, ((_, lb), keys)) in layers.iter().zip(&keys).enumerate() {
            let areas = keys.iter().map(|(key, spec)| text_area(&self.buffers[key], spec, *lb));
            let viewport = if i == 0 { &self.scene_viewport } else { &self.surface_viewport };
            if let Err(e) = self.text_renderers[i].prepare(
                &self.device,
                &self.queue,
                &mut fonts.system,
                &mut self.atlas,
                viewport,
                areas,
                &mut self.swash,
            ) {
                eprintln!("text: {e}");
            }
        }

        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = begin_pass(&mut encoder, &self.scene.view);
            self.draw_batches(&mut pass, &self.scene_globals, &scene_batches);
            let _ = self.text_renderers[0].render(&self.atlas, &self.scene_viewport, &mut pass);
        }
        {
            let mut pass = begin_pass(&mut encoder, view);
            self.draw_batches(&mut pass, &self.surface_globals, &composite);
            for (i, batches) in ui_batches.iter().enumerate() {
                self.draw_batches(&mut pass, &self.surface_globals, batches);
                let _ = self.text_renderers[i + 1].render(&self.atlas, &self.surface_viewport, &mut pass);
            }
        }
        self.queue.submit(Some(encoder.finish()));
        self.atlas.trim();
    }

    fn draw_batches(&self, pass: &mut wgpu::RenderPass<'_>, globals: &Globals, batches: &[Batch]) {
        if batches.is_empty() {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &globals.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        for batch in batches {
            let group = match &batch.source {
                Source::White => &self.white,
                Source::Image(name) => match self.images.get(name) {
                    Some(image) => &image.bind_group,
                    None => continue,
                },
                Source::Scene => &self.scene.bind_group,
                Source::Previous => &self.previous.bind_group,
            };
            pass.set_bind_group(1, group, &[]);
            pass.draw(batch.range.clone(), 0..1);
        }
    }

    fn push_layer(&self, vertices: &mut Vec<Vertex>, layer: &[Draw], lb: Letterbox) -> Vec<Batch> {
        let mut batches: Vec<Batch> = Vec::new();
        for draw in layer {
            let (source, rect, top, bottom, radius, rotation) = match draw {
                Draw::Fill { rect, top, bottom, radius } => (Source::White, rect, *top, *bottom, *radius, 0.0),
                Draw::Image { name, rect, opacity, rotation } if self.images.contains_key(name) => {
                    let c = Color::WHITE.with_alpha(*opacity);
                    (Source::Image(name.clone()), rect, c, c, 0.0, *rotation)
                }
                _ => continue,
            };
            let rect = rect.scaled(lb.scale, lb.x, lb.y);
            push_batch(
                &mut batches,
                vertices,
                source,
                BatchStyle { rect, top, bottom, radius: radius * lb.scale, rotation },
            );
        }
        batches
    }

    fn push_composite(&self, vertices: &mut Vec<Vertex>) -> Vec<Batch> {
        let lb = self.letterbox;
        let rect = Rect::new(0.0, 0.0, self.virtual_size.0, self.virtual_size.1).scaled(lb.scale, lb.x, lb.y);
        let mut batches = Vec::new();
        let mut push = |source, alpha: f32| {
            let c = if source == Source::White { Color::BLACK } else { Color::WHITE }.with_alpha(alpha);
            push_batch(
                &mut batches,
                vertices,
                source,
                BatchStyle { rect, top: c, bottom: c, radius: 0.0, rotation: 0.0 },
            );
        };
        match &self.transition {
            None => push(Source::Scene, 1.0),
            Some(t) => {
                let p = (t.elapsed / t.duration).clamp(0.0, 1.0);
                match t.kind {
                    TransitionKind::Dissolve => {
                        push(Source::Previous, 1.0);
                        push(Source::Scene, p);
                    }
                    TransitionKind::Fade if p < 0.5 => {
                        push(Source::Previous, 1.0);
                        push(Source::White, p * 2.0);
                    }
                    TransitionKind::Fade => {
                        push(Source::Scene, 1.0);
                        push(Source::White, (1.0 - p) * 2.0);
                    }
                }
            }
        }
        batches
    }
}

struct BatchStyle {
    rect: Rect,
    top: Color,
    bottom: Color,
    radius: f32,
    rotation: f32,
}

fn push_batch(batches: &mut Vec<Batch>, vertices: &mut Vec<Vertex>, source: Source, style: BatchStyle) {
    let BatchStyle { rect, top, bottom, radius, rotation } = style;
    let start = vertices.len() as u32;
    let (top, bottom) = (top.linear(), bottom.linear());
    let r = [rect.x, rect.y, rect.w, rect.h];
    let center = (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
    let (sin, cos) = rotation.to_radians().sin_cos();
    let rotate = |x: f32, y: f32| {
        let dx = x - center.0;
        let dy = y - center.1;
        (center.0 + dx * cos - dy * sin, center.1 + dx * sin + dy * cos)
    };
    let v = |x: f32, y: f32, u: f32, w: f32, color| {
        let (x, y) = rotate(x, y);
        Vertex { pos: [x, y], uv: [u, w], color, rect: r, radius }
    };
    let (x0, y0, x1, y1) = (rect.x, rect.y, rect.x + rect.w, rect.y + rect.h);
    vertices.extend([
        v(x0, y0, 0.0, 0.0, top),
        v(x1, y0, 1.0, 0.0, top),
        v(x0, y1, 0.0, 1.0, bottom),
        v(x0, y1, 0.0, 1.0, bottom),
        v(x1, y0, 1.0, 0.0, top),
        v(x1, y1, 1.0, 1.0, bottom),
    ]);
    let end = vertices.len() as u32;
    match batches.last_mut() {
        Some(last) if last.source == source && last.range.end == start => last.range.end = end,
        _ => batches.push(Batch { source, range: start..end }),
    }
}

fn texts(layer: &[Draw]) -> impl Iterator<Item = &TextSpec> {
    layer.iter().filter_map(|d| match d {
        Draw::Text(spec) if !spec.text.is_empty() => Some(spec),
        _ => None,
    })
}

fn buffer_key(spec: &TextSpec, scale: f32) -> u64 {
    let mut h = DefaultHasher::new();
    (&spec.text, spec.size.to_bits(), spec.rect.w.to_bits(), scale.to_bits()).hash(&mut h);
    (&spec.family, spec.bold, spec.align, spec.reveal, spec.color).hash(&mut h);
    h.finish()
}

fn text_area<'a>(buffer: &'a glyphon::Buffer, spec: &TextSpec, lb: Letterbox) -> TextArea<'a> {
    let rect = spec.rect.scaled(lb.scale, lb.x, lb.y);
    let top = match spec.align {
        TextAlign::Left => rect.y,
        TextAlign::Center => {
            let height: f32 = buffer.layout_runs().map(|r| r.line_height).sum();
            rect.y + (rect.h - height) / 2.0
        }
    };
    TextArea {
        buffer,
        left: rect.x,
        top,
        scale: 1.0,
        bounds: TextBounds {
            left: rect.x.floor() as i32,
            top: rect.y.floor() as i32,
            right: (rect.x + rect.w).ceil() as i32,
            bottom: (rect.y + rect.h).ceil() as i32,
        },
        default_color: glyph_color(spec.color),
        custom_glyphs: &[],
    }
}

fn begin_pass<'e>(encoder: &'e mut wgpu::CommandEncoder, view: &wgpu::TextureView) -> wgpu::RenderPass<'e> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

fn scene_size(virtual_size: (f32, f32), lb: Letterbox) -> (u32, u32) {
    (((virtual_size.0 * lb.scale).round() as u32).max(1), ((virtual_size.1 * lb.scale).round() as u32).max(1))
}

fn vertex_buffer(device: &wgpu::Device, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("kairos vertices"),
        size,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn upload(device: &wgpu::Device, queue: &wgpu::Queue, size: (u32, u32), rgba: &[u8]) -> wgpu::Texture {
    let extent = wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("kairos image"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        rgba,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * size.0), rows_per_image: None },
        extent,
    );
    texture
}

fn texture_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    texture: &wgpu::Texture,
) -> wgpu::BindGroup {
    let view = texture.create_view(&Default::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    })
}

fn target(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    format: wgpu::TextureFormat,
    size: (u32, u32),
) -> Target {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("kairos scene"),
        size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let bind_group = texture_bind_group(device, layout, sampler, &texture);
    Target { texture, view, bind_group }
}
