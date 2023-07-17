use bevy::{
    prelude::*,
    render::{
        render_asset::RenderAssets,
        render_resource::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        },
        renderer::{RenderDevice, RenderQueue},
    },
};

use bytemuck;
use image;
use wgpu::ImageCopyBuffer;
use wgpu::ImageDataLayout;

use crate::state;

fn texture_image_layout(desc: &TextureDescriptor<'_>) -> ImageDataLayout {
    let size = desc.size;

    let width = size.width * desc.format.block_dimensions().0;
    let height = size.width * desc.format.block_dimensions().1;

    ImageDataLayout {
        bytes_per_row: if size.height > 1 { Some(width) } else { None },
        rows_per_image: if size.depth_or_array_layers > 1 {
            Some(height)
        } else {
            None
        },
        ..Default::default()
    }
}

/// Copy a texture buffer from GPU to RAM and convert color space to RGBA.
/// It makes possible to export render results via API.
pub(crate) fn copy_from_gpu_to_ram<
    T: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe,
    P: 'static + Send + Sync + Clone + std::panic::RefUnwindSafe + serde::Serialize,
>(
    gpu_images: Res<RenderAssets<Image>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    ai_gym_state: Res<state::AIGymState<T, P>>,
) {
    let mut ai_gym_state_locked = ai_gym_state.lock().unwrap();
    if !ai_gym_state_locked.settings.render_to_buffer {
        return;
    }
    let ai_gym_settings = ai_gym_state_locked.settings.clone();

    let device = render_device.wgpu_device();
    let size = Extent3d {
        width: ai_gym_settings.width,
        height: ai_gym_settings.height,
        ..default()
    };

    ai_gym_state_locked.visual_observations = Vec::new();
    for (_, gp) in ai_gym_state_locked
        .render_image_handles
        .clone()
        .iter()
        .enumerate()
    {
        let render_gpu_image = gpu_images.get(gp).unwrap();
        let texture_width = size.width;
        let texture_height = size.height;

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
                    view_formats: &[TextureFormat::Bgra8UnormSrgb],
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
            image::ImageBuffer::from_raw(texture_width, texture_height, result.clone()).unwrap();

        // fixing bgra to rgba
        convert_bgra_to_rgba(&mut rgba_image);

        ai_gym_state_locked
            .visual_observations
            .push(rgba_image.clone());

        destination.unmap();
    }
}

/// convert BRGA image to RGBA image
fn convert_bgra_to_rgba(image: &mut image::RgbaImage) {
    for pixel in image.pixels_mut() {
        pixel.0.swap(0, 2);
    }
}
