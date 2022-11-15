use std::marker::PhantomData;
use std::num::NonZeroU32;
use std::thread;

use bevy::render::view::RenderLayers;
use bevy::{
    prelude::*,
    render::{
        render_asset::RenderAssets,
        render_resource::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        },
        renderer::{RenderDevice, RenderQueue},
        RenderApp, RenderStage,
    },
};

use bytemuck;
use image;
use wgpu::ImageCopyBuffer;
use wgpu::ImageDataLayout;

use crate::{api, state};

#[derive(Clone, Resource)]
pub struct AIGymSettings {
    pub width: u32,
    pub height: u32,
    pub num_agents: u32,
}

#[derive(Default, Clone)]
pub struct AIGymPlugin<
    T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
>(pub PhantomData<(T, P)>);

// ----------
// Components
// ----------

#[derive(Component)]
struct Interface;

impl<
        T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
        P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
    > Plugin for AIGymPlugin<T, P>
{
    fn build(&self, app: &mut App) {
        app.add_startup_system(setup::<T, P>.label("setup_rendering"));

        let ai_gym_state = app
            .world
            .get_resource::<state::AIGymState<T, P>>()
            .unwrap()
            .clone();

        let ai_gym_settings = app.world.get_resource::<AIGymSettings>().unwrap().clone();

        if let Ok(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_system_to_stage(RenderStage::Render, copy_from_gpu_to_ram::<T, P>);
            render_app.insert_resource(ai_gym_state);
            render_app.insert_resource(ai_gym_settings);
        }
    }
}

// ------------------
// Rendering to Image
// ------------------

fn texture_image_layout(desc: &TextureDescriptor<'_>) -> ImageDataLayout {
    let size = desc.size;

    ImageDataLayout {
        bytes_per_row: if size.height > 1 {
            NonZeroU32::new(size.width * (desc.format.describe().block_size as u32))
        } else {
            None
        },
        rows_per_image: if size.depth_or_array_layers > 1 {
            NonZeroU32::new(size.height)
        } else {
            None
        },
        ..Default::default()
    }
}

fn copy_from_gpu_to_ram<
    T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
>(
    gpu_images: Res<RenderAssets<Image>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    ai_gym_state: Res<state::AIGymState<T, P>>,
    ai_gym_settings: Res<AIGymSettings>,
) {
    let mut ai_gym_state_locked = ai_gym_state.lock().unwrap();
    let device = render_device.wgpu_device();
    let size = Extent3d {
        width: ai_gym_settings.width,
        height: ai_gym_settings.height,
        ..default()
    };

    ai_gym_state_locked.visual_observations = Vec::new();
    for (_i, gp) in ai_gym_state_locked
        .render_image_handles
        .clone()
        .iter()
        .enumerate()
    {
        let render_gpu_image = gpu_images.get(gp).unwrap();
        let texture_width = size.width as u32;
        let texture_height = size.height as u32;

        let destination = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (texture_width * texture_height * 4) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let texture = render_gpu_image.texture.clone();

        let mut encoder =
            render_device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let texture_extent = Extent3d {
            width: texture_width,
            height: texture_height,
            ..default()
        };

        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            ImageCopyBuffer {
                buffer: &destination,
                layout: texture_image_layout(&TextureDescriptor {
                    label: None,
                    size,
                    dimension: TextureDimension::D2,
                    format: TextureFormat::Bgra8UnormSrgb,
                    mip_level_count: 1,
                    sample_count: 1,
                    usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                }),
            },
            texture_extent,
        );

        render_queue.submit([encoder.finish()]);
        let buffer_slice = destination.slice(..);

        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let err = result.err();
            if err.is_some() {
                panic!("{}", err.unwrap().to_string());
            }
        });

        device.poll(wgpu::Maintain::Wait);

        let data = buffer_slice.get_mapped_range();
        let result: Vec<u8> = bytemuck::cast_slice(&data).to_vec();

        drop(data);
        let mut rgba_image: image::RgbaImage =
            image::ImageBuffer::from_raw(texture_width, texture_height as u32, result.clone())
                .unwrap();

        // fixing bgra to rgba
        convert_bgra_to_rgba(&mut rgba_image);

        ai_gym_state_locked
            .visual_observations
            .push(rgba_image.clone());

        destination.unmap();
    }
}

// convert BRGA image to RGBA image
fn convert_bgra_to_rgba(image: &mut image::RgbaImage) {
    for pixel in image.pixels_mut() {
        let b = pixel[0];
        let g = pixel[1];
        let r = pixel[2];
        let a = pixel[3];
        pixel[0] = r;
        pixel[1] = g;
        pixel[2] = b;
        pixel[3] = a;
    }
}

fn setup<
    T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
>(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    ai_gym_state: ResMut<state::AIGymState<T, P>>,
    ai_gym_settings: Res<AIGymSettings>,
    mut windows: ResMut<Windows>,
) {
    let size = Extent3d {
        width: ai_gym_settings.width,
        height: ai_gym_settings.height,
        ..default()
    };

    let ai_gym_state_1 = ai_gym_state.into_inner().clone();
    let ai_gym_state_2 = ai_gym_state_1.clone();

    let mut ai_gym_state = ai_gym_state_1.lock().unwrap();

    for _ in 0..ai_gym_settings.num_agents {
        // This is the texture that will be rendered to.
        let mut render_image = Image {
            texture_descriptor: TextureDescriptor {
                label: None,
                size,
                dimension: TextureDimension::D2,
                format: TextureFormat::Bgra8UnormSrgb,
                mip_level_count: 1,
                sample_count: 1,
                usage: TextureUsages::COPY_SRC
                    | TextureUsages::COPY_DST
                    | TextureUsages::TEXTURE_BINDING
                    | TextureUsages::RENDER_ATTACHMENT,
            },
            ..default()
        };
        render_image.resize(size);
        ai_gym_state
            .render_image_handles
            .push(images.add(render_image));

        let mut display_image = Image {
            texture_descriptor: TextureDescriptor {
                label: None,
                size,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8UnormSrgb,
                mip_level_count: 1,
                sample_count: 1,
                usage: TextureUsages::COPY_SRC
                    | TextureUsages::COPY_DST
                    | TextureUsages::TEXTURE_BINDING
                    | TextureUsages::RENDER_ATTACHMENT,
            },
            ..default()
        };
        display_image.resize(size);
    }

    let ai_gym_settings = ai_gym_settings.clone();
    let num_agents = ai_gym_settings.num_agents;

    thread::spawn(move || {
        gotham::start(
            "127.0.0.1:7878",
            api::router::<T, P>(api::GothamState {
                inner: ai_gym_state_2,
                settings: ai_gym_settings,
            }),
        )
    });

    let second_pass_layer = RenderLayers::layer(1);

    commands
        .spawn(Camera2dBundle::default())
        .insert(second_pass_layer);

    let window = windows.get_primary_mut().unwrap();
    let number_of_columns = (num_agents as f32).sqrt().ceil() as u32;
    let number_of_rows = ((num_agents as f32) / (number_of_columns as f32)).ceil() as u32;
    let mut frames: Vec<Handle<Image>> = Vec::new();
    for f in ai_gym_state.render_image_handles.iter() {
        frames.push(f.clone());
    }
    let offset_x = (size.width * number_of_rows / 2 - size.width / 2) as f32;
    let offset_y = (size.height * number_of_columns / 2 - size.height / 2) as f32;

    for r in 0..number_of_rows {
        for c in 0..number_of_columns {
            let y = (r * size.height) as f32;
            let x = (c * size.width) as f32;

            let i = (c * number_of_columns + r) as usize;
            if i > (frames.len() - 1) {
                continue;
            }

            commands
                .spawn(SpriteBundle {
                    texture: frames[i].clone(),
                    transform: Transform::from_xyz(x - offset_x, y - offset_y, 0.0),
                    ..default()
                })
                .insert(second_pass_layer);
        }
    }

    window.set_resolution(
        (size.width * number_of_rows) as f32,
        (size.height * number_of_columns) as f32,
    );
    window.set_resizable(false);
}
