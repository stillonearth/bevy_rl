#![feature(associated_type_bounds)]

// use std::io::Cursor;
use std::marker::PhantomData;
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
use std::thread;

use bevy::core_pipeline::core_3d::graph::node;
use bevy::core_pipeline::core_3d::{AlphaMask3d, Opaque3d, Transparent3d};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_graph::RenderGraph;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::{RenderApp, RenderStage};
use bevy::{
    prelude::*,
    render::{
        camera::RenderTarget,
        render_resource::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        },
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
}

#[derive(Default, Clone)]
pub struct AIGymPlugin<T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe>(
    pub PhantomData<T>,
);

impl<T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe> Plugin for AIGymPlugin<T> {
    fn build(&self, app: &mut App) {
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
    let gp: Handle<Image> = ai_gym_state_locked.__render_image_handle.clone().unwrap();

    let gpu_image = gpu_images.get(&gp).unwrap();
    let texture_width = gpu_image.size.x as u32;
    let texture_height = gpu_image.size.y as u32;

    let device = render_device.wgpu_device();

    let destination = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (gpu_image.size.x * gpu_image.size.y * 4.0) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let texture = gpu_image.texture.clone();

    let mut encoder =
        render_device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

    let size = Extent3d {
        width: ai_gym_settings.width,
        height: ai_gym_settings.height,
        ..default()
    };

    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        ImageCopyBuffer {
            buffer: &destination,
            layout: texture_image_layout(&TextureDescriptor {
                label: Some("render_image"),
                size,
                dimension: TextureDimension::D2,
                format: TextureFormat::Bgra8UnormSrgb,
                mip_level_count: 1,
                sample_count: 1,
                usage: TextureUsages::TEXTURE_BINDING
                    | TextureUsages::COPY_DST
                    | TextureUsages::RENDER_ATTACHMENT, // | TextureUsages::STORAGE_BINDING,
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
    let img: image::RgbaImage =
        image::ImageBuffer::from_raw(texture_width, texture_height as u32, result).unwrap();

    ai_gym_state_locked.screen = Some(img.clone());

    destination.unmap();
}

fn setup<T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe>(
    mut images: ResMut<Assets<Image>>,
    ai_gym_state: ResMut<Arc<Mutex<state::AIGymState<T>>>>,
    ai_gym_settings: Res<AIGymSettings>,
) {
    let size = Extent3d {
        width: ai_gym_settings.width,
        height: ai_gym_settings.height,
        ..default()
    };

    // This is the texture that will be rendered to.
    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("render_image"),
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb, // ::Bgra8UnormSrgb,
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

    let image_handle = images.add(image);

    let ai_gym_state_1 = ai_gym_state.into_inner().clone();
    let ai_gym_state_2 = ai_gym_state_1.clone();

    let mut ai_gym_state_ = ai_gym_state_1.lock().unwrap();

    ai_gym_state_.__render_target = Some(RenderTarget::Image(image_handle.clone()));
    ai_gym_state_.__render_image_handle = Some(image_handle.clone());

    thread::spawn(move || {
        gotham::start(
            "127.0.0.1:7878",
            api::router::<T>(api::GothamState {
                inner: ai_gym_state_2,
            }),
        )
    });
}
