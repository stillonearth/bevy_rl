#![feature(associated_type_bounds)]

// use std::io::Cursor;
use std::marker::PhantomData;
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
use std::thread;

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

mod api;
pub mod state;

#[derive(Clone)]
pub struct AIGymSettings {
    pub width: u32,
    pub height: u32,
    pub num_agents: u32,
}

#[derive(Default, Clone)]
pub struct AIGymPlugin<T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe>(
    pub PhantomData<T>,
);

// ----------
// Components
// ----------

#[derive(Component)]
pub struct Interface;

impl<T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe> Plugin for AIGymPlugin<T> {
    fn build(&self, app: &mut App) {
        app.add_startup_system(setup::<T>.label("setup_rendering"));

        let ai_gym_state = app
            .world
            .get_resource::<Arc<Mutex<state::AIGymState<T>>>>()
            .unwrap()
            .clone();

        let ai_gym_settings = app.world.get_resource::<AIGymSettings>().unwrap().clone();

        if let Ok(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_system_to_stage(RenderStage::Render, save_image::<T>);
            render_app.insert_resource(ai_gym_state.clone());
            render_app.insert_resource(ai_gym_settings.clone());
        }
    }
}

// ------------------
// Rendering to Image
// ------------------

pub fn texture_image_layout(desc: &TextureDescriptor<'_>) -> ImageDataLayout {
    let size = desc.size;

    let layout = ImageDataLayout {
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
    };

    return layout;
}

fn save_image<T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe>(
    gpu_images: Res<RenderAssets<Image>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    ai_gym_state: Res<Arc<Mutex<state::AIGymState<T>>>>,
    ai_gym_settings: Res<AIGymSettings>,
) {
    let mut ai_gym_state_locked = ai_gym_state.lock().unwrap();
    let device = render_device.wgpu_device();
    let size = Extent3d {
        width: ai_gym_settings.width,
        height: ai_gym_settings.height,
        ..default()
    };

    ai_gym_state_locked.screens = Vec::new();
    for gp in ai_gym_state_locked.render_image_handles.clone() {
        let gpu_image = gpu_images.get(&gp).unwrap();
        let texture_width = size.width as u32;
        let texture_height = size.height as u32;

        let destination = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (texture_width * texture_height * 4) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let texture = gpu_image.texture.clone();

        let mut encoder =
            render_device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

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
                    usage: TextureUsages::TEXTURE_BINDING
                        | TextureUsages::COPY_DST
                        | TextureUsages::RENDER_ATTACHMENT,
                }),
            },
            Extent3d {
                width: texture_width,
                height: texture_height,
                ..default()
            },
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

        ai_gym_state_locked.screens.push(rgba_image.clone());
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

fn setup<T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe>(
    mut images: ResMut<Assets<Image>>,
    ai_gym_state: ResMut<Arc<Mutex<state::AIGymState<T>>>>,
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
        let mut image = Image {
            texture_descriptor: TextureDescriptor {
                label: None,
                size,
                dimension: TextureDimension::D2,
                format: TextureFormat::Bgra8UnormSrgb,
                mip_level_count: 1,
                sample_count: 1,
                usage: TextureUsages::TEXTURE_BINDING
                    | TextureUsages::COPY_SRC
                    | TextureUsages::COPY_DST
                    | TextureUsages::RENDER_ATTACHMENT,
            },
            ..default()
        };

        // fill image.data with zeroes
        image.resize(size);
        ai_gym_state.render_image_handles.push(images.add(image));
    }

    let ai_gym_settings = ai_gym_settings.clone();
    thread::spawn(move || {
        gotham::start(
            "127.0.0.1:7878",
            api::router::<T>(api::GothamState {
                inner: ai_gym_state_2,
                settings: ai_gym_settings,
            }),
        )
    });

    let window = windows.get_primary_mut().unwrap();
    window.set_resolution(size.width as f32, size.height as f32);
    window.set_resizable(false);
}
