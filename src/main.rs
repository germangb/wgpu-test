use bytemuck::{Pod, Zeroable};
use log::{info, LevelFilter};
use sdl2::event::{Event, WindowEvent};
use wgpu::{
    include_spirv,
    util::{BufferInitDescriptor, DeviceExt},
    vertex_attr_array, BackendBit, BlendDescriptor, BufferUsage, Color, ColorStateDescriptor,
    ColorWrite, CommandEncoderDescriptor, CullMode, DeviceDescriptor, FrontFace, IndexFormat,
    InputStepMode, Instance, LoadOp, Operations, PipelineLayoutDescriptor, PowerPreference,
    PresentMode, PrimitiveTopology, ProgrammableStageDescriptor, RasterizationStateDescriptor,
    RenderPassColorAttachmentDescriptor, RenderPassDescriptor, RenderPipelineDescriptor,
    RequestAdapterOptions, SwapChainDescriptor, TextureFormat, TextureUsage,
    VertexBufferDescriptor, VertexStateDescriptor,
};

const WIDTH: usize = 640;
const HEIGHT: usize = 480;

fn main() {
    env_logger::builder()
        .filter(Some("gfx_backend_vulkan"), LevelFilter::Warn)
        .filter(Some("gfx_memory"), LevelFilter::Warn)
        .init();

    let sdl = sdl2::init().unwrap();
    let mut events = sdl.event_pump().unwrap();

    // init window
    let video = sdl.video().unwrap();
    let window = video
        .window("wgpu", WIDTH as _, HEIGHT as _)
        .position_centered()
        .build()
        .unwrap();

    // init web gpu
    let instance = Instance::new(BackendBit::VULKAN);
    let surface = unsafe { instance.create_surface(&window) };
    let adapter = futures::executor::block_on(instance.request_adapter(&RequestAdapterOptions {
        power_preference: PowerPreference::Default,
        compatible_surface: Some(&surface),
    }))
    .expect("Couldn't create adapter");
    info!("Adapter info: {:?}", adapter.get_info());
    info!("Adapter features: {:?}", adapter.features());
    info!("Adapter limits: {:?}", adapter.limits());

    // init device and swap chain.
    let (device, queue) = futures::executor::block_on(adapter.request_device(
        &DeviceDescriptor {
            shader_validation: true,
            ..Default::default()
        },
        None,
    ))
    .expect("Error requesting device");
    info!("Device limits: {:?}", device.limits());
    info!("Device features: {:?}", device.features());

    let mut swap_chain = device.create_swap_chain(
        &surface,
        &SwapChainDescriptor {
            usage: TextureUsage::OUTPUT_ATTACHMENT,
            format: TextureFormat::Bgra8UnormSrgb,
            width: WIDTH as _,
            height: HEIGHT as _,
            present_mode: PresentMode::Fifo,
        },
    );

    // Mesh data buffers.
    #[repr(C)]
    #[derive(Clone, Copy, Pod, Zeroable)]
    struct Vertex {
        _pos: [f32; 2],
        _color: [f32; 3],
    }

    #[rustfmt::skip]
    let vertex = device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        contents: bytemuck::bytes_of(&[
            Vertex { _pos: [0.0, 0.0], _color: [1.0, 0.0, 0.0] },
            Vertex { _pos: [1.0, 0.0], _color: [0.0, 1.0, 0.0] },
            Vertex { _pos: [0.0, 1.0], _color: [0.0, 0.0, 1.0] },
        ]),
        usage: BufferUsage::VERTEX,
    });

    let index = device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        contents: bytemuck::bytes_of(&[0u16, 1, 2]),
        usage: BufferUsage::INDEX,
    });

    // shaders
    let vert_module = device.create_shader_module(include_spirv!("shader.vert.spv"));
    let frag_module = device.create_shader_module(include_spirv!("shader.frag.spv"));
    // render pipeline and bind groups
    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });
    let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        vertex_stage: ProgrammableStageDescriptor {
            module: &vert_module,
            entry_point: "main",
        },
        fragment_stage: Some(ProgrammableStageDescriptor {
            module: &frag_module,
            entry_point: "main",
        }),
        rasterization_state: Some(RasterizationStateDescriptor {
            front_face: FrontFace::Ccw,
            cull_mode: CullMode::None,
            clamp_depth: false,
            depth_bias: 0,
            depth_bias_slope_scale: 0.0,
            depth_bias_clamp: 0.0,
        }),
        primitive_topology: PrimitiveTopology::TriangleList,
        color_states: &[ColorStateDescriptor {
            format: TextureFormat::Bgra8UnormSrgb,
            alpha_blend: BlendDescriptor::default(),
            color_blend: BlendDescriptor::default(),
            write_mask: ColorWrite::default(),
        }],
        depth_stencil_state: None,
        vertex_state: VertexStateDescriptor {
            index_format: IndexFormat::Uint16,
            vertex_buffers: &[VertexBufferDescriptor {
                stride: std::mem::size_of::<[f32; 2]>() as _,
                step_mode: InputStepMode::Vertex,
                attributes: &vertex_attr_array![0 => Float2, 1 => Float3][..],
            }],
        },
        sample_count: 1,
        sample_mask: !0,
        alpha_to_coverage_enabled: false,
    });

    // init imgui
    let mut imgui = imgui::Context::create();
    let mut imgui_sdl2 = imgui_sdl2::ImguiSdl2::new(&mut imgui, &window);
    let mut imgui_wgpu = imgui_wgpu::Renderer::new(
        &mut imgui,
        &device,
        &queue,
        imgui_wgpu::RendererConfig {
            texture_format: TextureFormat::Bgra8UnormSrgb,
            ..Default::default()
        },
    );

    'main: loop {
        for event in events.poll_iter() {
            match event {
                Event::Window {
                    win_event: WindowEvent::Close,
                    ..
                } => break 'main,
                _ if !imgui_sdl2.ignore_event(&event) => {
                    imgui_sdl2.handle_event(&mut imgui, &event);
                }
                _ => {}
            }
        }

        let frame = swap_chain
            .get_current_frame()
            .expect("Error getting current frame");

        // draw wgpu

        let mut cmd = device.create_command_encoder(&CommandEncoderDescriptor::default());
        {
            let mut pass = cmd.begin_render_pass(&RenderPassDescriptor {
                color_attachments: &[RenderPassColorAttachmentDescriptor {
                    attachment: &frame.output.view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color {
                            r: 0.5,
                            g: 0.5,
                            b: 0.5,
                            a: 1.0,
                        }),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });
            pass.set_pipeline(&render_pipeline);
            pass.set_vertex_buffer(0, vertex.slice(..));
            pass.set_index_buffer(index.slice(..));
            pass.draw(0..3, 0..1);
        }

        {
            let mut pass = cmd.begin_render_pass(&RenderPassDescriptor {
                color_attachments: &[RenderPassColorAttachmentDescriptor {
                    attachment: &frame.output.view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });

            // draw imgui
            imgui_sdl2.prepare_frame(imgui.io_mut(), &window, &events.mouse_state());
            let ui = imgui.frame();

            ui.show_demo_window(&mut true);
            ui.show_metrics_window(&mut true);

            imgui_sdl2.prepare_render(&ui, &window);
            imgui_wgpu
                .render(ui.render(), &queue, &device, &mut pass)
                .expect("Error rendering imgui");
        }

        queue.submit(Some(cmd.finish()));

        //std::thread::sleep(std::time::Duration::new(0, 1_000_000_000 / 60));
    }
}
