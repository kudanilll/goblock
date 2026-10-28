//! Sun shadows: a depth map rendered from the sun every frame, which the
//! block shaders compare against.
//!
//! Shadows cost a second pass over the nearby geometry, so their resolution
//! follows the device, and devices without a real GPU skip them. The map is
//! then a 1x1 image that is still cleared every frame, which keeps the
//! descriptors and the shaders the same either way.

use ash::vk;
use glam::{DMat4, DVec3, Mat4, Vec3};

use super::device::Device;
use super::error::VkError;
use super::swapchain::find_memory_type;

/// How far the shadow map reaches along the sunlight on either side of the
/// camera, in blocks, so terrain well above or below the camera still casts
/// shadows into it.
const DEPTH_REACH: f32 = 400.0;

pub struct ShadowMap {
    /// Edge length in texels, or 0 when shadows are off.
    pub size: u32,
    image: vk::Image,
    memory: vk::DeviceMemory,
    pub view: vk::ImageView,
    pub sampler: vk::Sampler,
    pub render_pass: vk::RenderPass,
    framebuffer: vk::Framebuffer,
}

impl ShadowMap {
    pub fn new(device: &Device) -> Result<Self, VkError> {
        let size = shadow_size(device.properties.device_type);
        let extent = size.max(1);
        let (format, linear) = pick_format(device);

        let info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .extent(vk::Extent3D {
                width: extent,
                height: extent,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);
        let image = unsafe { device.device.create_image(&info, None) }
            .map_err(|e| VkError::new("create_shadow_image", e))?;

        let req = unsafe { device.device.get_image_memory_requirements(image) };
        let memory_type = find_memory_type(
            device,
            req.memory_type_bits,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;
        let alloc = vk::MemoryAllocateInfo::default()
            .allocation_size(req.size)
            .memory_type_index(memory_type);
        let memory = unsafe { device.device.allocate_memory(&alloc, None) }
            .map_err(|e| VkError::new("allocate_memory", e))?;
        unsafe { device.device.bind_image_memory(image, memory, 0) }
            .map_err(|e| VkError::new("bind_image_memory", e))?;

        let view_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::DEPTH)
                    .level_count(1)
                    .layer_count(1),
            );
        let view = unsafe { device.device.create_image_view(&view_info, None) }
            .map_err(|e| VkError::new("create_image_view", e))?;

        // Comparison sampling turns each lookup into lit (1) or shadowed (0).
        // Outside the map the border reads as the far plane, so it is lit.
        let filter = if linear {
            vk::Filter::LINEAR
        } else {
            vk::Filter::NEAREST
        };
        let sampler_info = vk::SamplerCreateInfo::default()
            .mag_filter(filter)
            .min_filter(filter)
            .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_BORDER)
            .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_BORDER)
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_BORDER)
            .border_color(vk::BorderColor::FLOAT_OPAQUE_WHITE)
            .compare_enable(true)
            .compare_op(vk::CompareOp::LESS_OR_EQUAL);
        let sampler = unsafe { device.device.create_sampler(&sampler_info, None) }
            .map_err(|e| VkError::new("create_sampler", e))?;

        let render_pass = create_render_pass(device, format)?;

        let attachments = [view];
        let framebuffer_info = vk::FramebufferCreateInfo::default()
            .render_pass(render_pass)
            .attachments(&attachments)
            .width(extent)
            .height(extent)
            .layers(1);
        let framebuffer = unsafe { device.device.create_framebuffer(&framebuffer_info, None) }
            .map_err(|e| VkError::new("create_framebuffer", e))?;

        Ok(Self {
            size,
            image,
            memory,
            view,
            sampler,
            render_pass,
            framebuffer,
        })
    }

    /// Distance between neighbouring texels in shadow map coordinates, or 0
    /// when shadows are off, which tells the shaders to skip the lookup.
    pub fn texel(&self) -> f32 {
        if self.size == 0 {
            0.0
        } else {
            1.0 / self.size as f32
        }
    }

    /// Begin the shadow render pass, cleared to the far plane, with the
    /// viewport covering the whole map.
    ///
    /// # Safety
    /// `cmd` must be recording, outside a render pass.
    pub unsafe fn begin(&self, device: &Device, cmd: vk::CommandBuffer) {
        let extent = self.size.max(1);
        let area = vk::Rect2D::default().extent(vk::Extent2D {
            width: extent,
            height: extent,
        });
        let clear = [vk::ClearValue {
            depth_stencil: vk::ClearDepthStencilValue {
                depth: 1.0,
                stencil: 0,
            },
        }];
        let info = vk::RenderPassBeginInfo::default()
            .render_pass(self.render_pass)
            .framebuffer(self.framebuffer)
            .render_area(area)
            .clear_values(&clear);
        let d = &device.device;
        d.cmd_begin_render_pass(cmd, &info, vk::SubpassContents::INLINE);
        let viewport = vk::Viewport::default()
            .width(extent as f32)
            .height(extent as f32)
            .max_depth(1.0);
        d.cmd_set_viewport(cmd, 0, &[viewport]);
        d.cmd_set_scissor(cmd, 0, &[area]);
    }

    /// # Safety
    /// The GPU must be done with the shadow map.
    pub unsafe fn destroy(&mut self, device: &Device) {
        let d = &device.device;
        d.destroy_framebuffer(self.framebuffer, None);
        d.destroy_render_pass(self.render_pass, None);
        d.destroy_sampler(self.sampler, None);
        d.destroy_image_view(self.view, None);
        d.destroy_image(self.image, None);
        d.free_memory(self.memory, None);
    }
}

/// Shadow map edge length for a device, or 0 for no shadows.
fn shadow_size(device_type: vk::PhysicalDeviceType) -> u32 {
    match device_type {
        vk::PhysicalDeviceType::DISCRETE_GPU => 2048,
        vk::PhysicalDeviceType::INTEGRATED_GPU => 1024,
        // Virtual and software devices are too slow for a second pass.
        _ => 0,
    }
}

/// D32_SFLOAT when the device can both render and sample it, otherwise
/// D16_UNORM, which every Vulkan 1.0 device must support for both. Also
/// reports whether comparisons can be filtered linearly.
fn pick_format(device: &Device) -> (vk::Format, bool) {
    let needed =
        vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT | vk::FormatFeatureFlags::SAMPLED_IMAGE;
    let features = |format| {
        unsafe {
            device
                .instance
                .get_physical_device_format_properties(device.physical, format)
        }
        .optimal_tiling_features
    };
    let format = if features(vk::Format::D32_SFLOAT).contains(needed) {
        vk::Format::D32_SFLOAT
    } else {
        vk::Format::D16_UNORM
    };
    let linear = features(format).contains(vk::FormatFeatureFlags::SAMPLED_IMAGE_FILTER_LINEAR);
    (format, linear)
}

fn create_render_pass(device: &Device, format: vk::Format) -> Result<vk::RenderPass, VkError> {
    let attachments = [vk::AttachmentDescription::default()
        .format(format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    let depth_ref = vk::AttachmentReference {
        attachment: 0,
        layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
    };
    let subpasses = [vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .depth_stencil_attachment(&depth_ref)];

    let depth_tests =
        vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS;
    let dependencies = [
        // The previous frame's block shaders must finish reading the map
        // before this frame clears it.
        vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
            .dst_stage_mask(depth_tests)
            .dst_access_mask(
                vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            ),
        // And this frame's block shaders read it only once it is written.
        vk::SubpassDependency::default()
            .src_subpass(0)
            .dst_subpass(vk::SUBPASS_EXTERNAL)
            .src_stage_mask(vk::PipelineStageFlags::LATE_FRAGMENT_TESTS)
            .src_access_mask(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
            .dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
            .dst_access_mask(vk::AccessFlags::SHADER_READ),
    ];

    let info = vk::RenderPassCreateInfo::default()
        .attachments(&attachments)
        .subpasses(&subpasses)
        .dependencies(&dependencies);
    unsafe { device.device.create_render_pass(&info, None) }
        .map_err(|e| VkError::new("create_render_pass", e))
}

/// Map camera-relative positions into the sun's shadow map: an orthographic
/// box reaching `radius` blocks to each side of the camera across the
/// sunlight. The box only moves in whole shadow texels, so shadow edges stay
/// put while the camera moves instead of shimmering.
pub fn shadow_matrix(camera: Vec3, sun: Vec3, radius: f32, size: u32) -> Mat4 {
    let up = if sun.y.abs() > 0.99 {
        DVec3::Z
    } else {
        DVec3::Y
    };
    let rotation = DMat4::look_to_rh(DVec3::ZERO, -sun.as_dvec3(), up);
    let centre = rotation.transform_point3(camera.as_dvec3());
    let texel = 2.0 * radius as f64 / size.max(1) as f64;
    let snapped = DVec3::new(
        (centre.x / texel).floor() * texel,
        (centre.y / texel).floor() * texel,
        centre.z,
    );

    // A camera-relative point q lands at R q + (R camera - snapped) in light
    // space. That difference is small but its terms are not, so it is taken
    // in f64 to stay exact far from the origin.
    let view = Mat4::from_translation((centre - snapped).as_vec3()) * rotation.as_mat4();
    let depth = radius + DEPTH_REACH;
    Mat4::orthographic_rh(-radius, radius, -radius, radius, -depth, depth) * view
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sun() -> Vec3 {
        Vec3::new(0.55, 0.62, 0.35).normalize()
    }

    #[test]
    fn nearby_points_land_inside_the_map() {
        let camera = Vec3::new(12_345.6, 90.0, -7_000.25);
        let m = shadow_matrix(camera, sun(), 160.0, 2048);
        // Anything within the radius of the camera, in any direction.
        for offset in [
            Vec3::ZERO,
            Vec3::new(100.0, -60.0, 50.0),
            Vec3::new(-90.0, 110.0, -70.0),
            Vec3::new(0.0, -159.0, 0.0),
        ] {
            let p = m.project_point3(offset);
            assert!(p.x.abs() <= 1.0 && p.y.abs() <= 1.0, "{offset} -> {p}");
            assert!((0.0..=1.0).contains(&p.z), "{offset} -> {p}");
        }
    }

    #[test]
    fn the_map_moves_in_whole_texels() {
        let size = 2048;
        let world_point = Vec3::new(5_000.0, 70.0, 3_000.0);
        let first = Vec3::new(5_010.0, 80.0, 3_005.0);
        let second = first + Vec3::new(0.37, 0.11, -0.52);
        let a = shadow_matrix(first, sun(), 160.0, size).project_point3(world_point - first);
        let b = shadow_matrix(second, sun(), 160.0, size).project_point3(world_point - second);
        // Clip space spans 2 units across `size` texels.
        for shift in [
            (a.x - b.x) * size as f32 / 2.0,
            (a.y - b.y) * size as f32 / 2.0,
        ] {
            assert!(
                (shift - shift.round()).abs() < 0.02,
                "shifted by {shift} texels"
            );
        }
    }

    #[test]
    fn overhead_sun_does_not_break_the_matrix() {
        let m = shadow_matrix(Vec3::new(1.0, 2.0, 3.0), Vec3::Y, 100.0, 1024);
        assert!(m.is_finite());
    }

    #[test]
    fn weak_devices_skip_shadows() {
        assert_eq!(shadow_size(vk::PhysicalDeviceType::CPU), 0);
        assert_eq!(shadow_size(vk::PhysicalDeviceType::VIRTUAL_GPU), 0);
        assert!(shadow_size(vk::PhysicalDeviceType::INTEGRATED_GPU) > 0);
    }
}
