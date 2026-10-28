//! GPU buffers, the font atlas texture, and pipeline creation helpers.

use ash::vk;

use super::device::Device;
use super::error::VkError;
use super::swapchain::find_memory_type;

/// A host-visible buffer that stays mapped. Holds per-frame vertex data, and
/// doubles as the staging buffer for texture uploads.
pub struct DynamicBuffer {
    pub buffer: vk::Buffer,
    pub memory: vk::DeviceMemory,
    pub size: u64,
    mapped: *mut u8,
}

impl DynamicBuffer {
    pub fn new(device: &Device, size: u64, usage: vk::BufferUsageFlags) -> Result<Self, VkError> {
        let info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let buffer = unsafe { device.device.create_buffer(&info, None) }
            .map_err(|e| VkError::new("create_buffer", e))?;

        let req = unsafe { device.device.get_buffer_memory_requirements(buffer) };
        let memory_type = find_memory_type(
            device,
            req.memory_type_bits,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;

        let alloc = vk::MemoryAllocateInfo::default()
            .allocation_size(req.size)
            .memory_type_index(memory_type);

        let memory = unsafe { device.device.allocate_memory(&alloc, None) }
            .map_err(|e| VkError::new("allocate_memory", e))?;

        unsafe { device.device.bind_buffer_memory(buffer, memory, 0) }
            .map_err(|e| VkError::new("bind_buffer_memory", e))?;

        let mapped = unsafe {
            device
                .device
                .map_memory(memory, 0, req.size, vk::MemoryMapFlags::empty())
        }
        .map_err(|e| VkError::new("map_memory", e))? as *mut u8;

        Ok(Self {
            buffer,
            memory,
            size: req.size,
            mapped,
        })
    }

    /// Copy `data` into the mapped buffer. Truncates if it does not fit.
    pub fn write<T: Copy>(&mut self, data: &[T]) {
        let byte_len = std::mem::size_of_val(data).min(self.size as usize);
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr() as *const u8, self.mapped, byte_len);
        }
    }

    /// # Safety
    /// Requires idle device.
    pub unsafe fn destroy(&mut self, device: &Device) {
        device.device.unmap_memory(self.memory);
        device.device.destroy_buffer(self.buffer, None);
        device.device.free_memory(self.memory, None);
    }
}

/// Vertex data written once at startup. It lives in host-visible memory,
/// which every Vulkan device has, so no staging copy is needed.
pub struct StaticBuffer {
    pub buffer: vk::Buffer,
    pub memory: vk::DeviceMemory,
    pub count: u32,
}

impl StaticBuffer {
    pub fn new<T: Copy>(
        device: &Device,
        data: &[T],
        usage: vk::BufferUsageFlags,
    ) -> Result<Self, VkError> {
        let size = std::mem::size_of_val(data) as u64;
        if size == 0 {
            return Ok(Self {
                buffer: vk::Buffer::null(),
                memory: vk::DeviceMemory::null(),
                count: 0,
            });
        }

        let info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let buffer = unsafe { device.device.create_buffer(&info, None) }
            .map_err(|e| VkError::new("create_buffer", e))?;

        let req = unsafe { device.device.get_buffer_memory_requirements(buffer) };
        let memory_type = find_memory_type(
            device,
            req.memory_type_bits,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;

        let alloc = vk::MemoryAllocateInfo::default()
            .allocation_size(req.size)
            .memory_type_index(memory_type);

        let memory = unsafe { device.device.allocate_memory(&alloc, None) }
            .map_err(|e| VkError::new("allocate_memory", e))?;

        unsafe { device.device.bind_buffer_memory(buffer, memory, 0) }
            .map_err(|e| VkError::new("bind_buffer_memory", e))?;

        let mapped = unsafe {
            device
                .device
                .map_memory(memory, 0, req.size, vk::MemoryMapFlags::empty())
        }
        .map_err(|e| VkError::new("map_memory", e))? as *mut u8;

        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr() as *const u8, mapped, size as usize);
            device.device.unmap_memory(memory);
        }

        Ok(Self {
            buffer,
            memory,
            count: data.len() as u32,
        })
    }

    /// # Safety
    /// Requires idle device.
    pub unsafe fn destroy(&mut self, device: &Device) {
        if self.buffer == vk::Buffer::null() {
            return;
        }
        device.device.destroy_buffer(self.buffer, None);
        device.device.free_memory(self.memory, None);
    }
}

/// Single-channel font atlas texture plus its sampler.
pub struct Texture {
    pub image: vk::Image,
    pub memory: vk::DeviceMemory,
    pub view: vk::ImageView,
    pub sampler: vk::Sampler,
}

impl Texture {
    /// Upload `pixels`, one byte per texel in tightly packed rows, and leave
    /// the image ready for sampling in fragment shaders.
    ///
    /// The data goes through a staging buffer into an optimally tiled image.
    /// Vulkan 1.0 guarantees R8_UNORM sampling with optimal tiling only;
    /// linear tiling support for it is optional.
    pub fn new(device: &Device, pixels: &[u8], width: u32, height: u32) -> Result<Self, VkError> {
        let info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::R8_UNORM)
            .extent(vk::Extent3D {
                width,
                height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);

        let image = unsafe { device.device.create_image(&info, None) }
            .map_err(|e| VkError::new("create_texture_image", e))?;

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

        let mut staging = DynamicBuffer::new(
            device,
            pixels.len() as u64,
            vk::BufferUsageFlags::TRANSFER_SRC,
        )?;
        staging.write(pixels);
        let uploaded = unsafe { upload(device, staging.buffer, image, width, height) };
        unsafe { staging.destroy(device) };
        uploaded?;

        let view = super::swapchain::create_image_view(
            device,
            image,
            vk::Format::R8_UNORM,
            vk::ImageAspectFlags::COLOR,
        )?;

        let sampler_info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::NEAREST)
            .min_filter(vk::Filter::NEAREST)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE);

        let sampler = unsafe { device.device.create_sampler(&sampler_info, None) }
            .map_err(|e| VkError::new("create_sampler", e))?;

        Ok(Self {
            image,
            memory,
            view,
            sampler,
        })
    }

    /// # Safety
    /// Requires idle device.
    pub unsafe fn destroy(&mut self, device: &Device) {
        device.device.destroy_sampler(self.sampler, None);
        device.device.destroy_image_view(self.view, None);
        device.device.destroy_image(self.image, None);
        device.device.free_memory(self.memory, None);
    }
}

/// Copy `buffer` into `image` and move the image to SHADER_READ_ONLY_OPTIMAL,
/// using a one-off command buffer. Blocks until the GPU is done.
///
/// # Safety
/// `image` must be a fresh, single-mip R8 image with TRANSFER_DST usage.
unsafe fn upload(
    device: &Device,
    buffer: vk::Buffer,
    image: vk::Image,
    width: u32,
    height: u32,
) -> Result<(), VkError> {
    let pool_info = vk::CommandPoolCreateInfo::default()
        .queue_family_index(device.queue_family)
        .flags(vk::CommandPoolCreateFlags::TRANSIENT);
    let pool = device
        .device
        .create_command_pool(&pool_info, None)
        .map_err(|e| VkError::new("create_command_pool", e))?;

    let alloc = vk::CommandBufferAllocateInfo::default()
        .command_pool(pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(1);
    let cmd = device
        .device
        .allocate_command_buffers(&alloc)
        .map_err(|e| VkError::new("allocate_command_buffers", e))?[0];

    let begin =
        vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    device
        .device
        .begin_command_buffer(cmd, &begin)
        .map_err(|e| VkError::new("begin_command_buffer", e))?;

    let range = vk::ImageSubresourceRange::default()
        .aspect_mask(vk::ImageAspectFlags::COLOR)
        .base_mip_level(0)
        .level_count(1)
        .base_array_layer(0)
        .layer_count(1);

    let to_transfer = vk::ImageMemoryBarrier::default()
        .old_layout(vk::ImageLayout::UNDEFINED)
        .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image)
        .subresource_range(range)
        .src_access_mask(vk::AccessFlags::empty())
        .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE);
    device.device.cmd_pipeline_barrier(
        cmd,
        vk::PipelineStageFlags::TOP_OF_PIPE,
        vk::PipelineStageFlags::TRANSFER,
        vk::DependencyFlags::empty(),
        &[],
        &[],
        &[to_transfer],
    );

    // Row length 0 means the buffer rows are tightly packed.
    let region = vk::BufferImageCopy::default()
        .image_subresource(
            vk::ImageSubresourceLayers::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .mip_level(0)
                .base_array_layer(0)
                .layer_count(1),
        )
        .image_extent(vk::Extent3D {
            width,
            height,
            depth: 1,
        });
    device.device.cmd_copy_buffer_to_image(
        cmd,
        buffer,
        image,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        &[region],
    );

    let to_shader = vk::ImageMemoryBarrier::default()
        .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
        .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image)
        .subresource_range(range)
        .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
        .dst_access_mask(vk::AccessFlags::SHADER_READ);
    device.device.cmd_pipeline_barrier(
        cmd,
        vk::PipelineStageFlags::TRANSFER,
        vk::PipelineStageFlags::FRAGMENT_SHADER,
        vk::DependencyFlags::empty(),
        &[],
        &[],
        &[to_shader],
    );

    device
        .device
        .end_command_buffer(cmd)
        .map_err(|e| VkError::new("end_command_buffer", e))?;

    let fence_info = vk::FenceCreateInfo::default();
    let fence = device
        .device
        .create_fence(&fence_info, None)
        .map_err(|e| VkError::new("create_fence", e))?;

    let cmds = [cmd];
    let submit = vk::SubmitInfo::default().command_buffers(&cmds);
    device
        .device
        .queue_submit(device.queue, &[submit], fence)
        .map_err(|e| VkError::new("queue_submit", e))?;
    device
        .device
        .wait_for_fences(&[fence], true, u64::MAX)
        .map_err(|e| VkError::new("wait_for_fences", e))?;

    device.device.destroy_fence(fence, None);
    device.device.destroy_command_pool(pool, None);
    Ok(())
}

/// Shaders and fixed-function settings for one graphics pipeline.
pub struct PipelineConfig {
    pub vs_bytes: &'static [u8],
    pub fs_bytes: &'static [u8],
    pub vertex_bindings: Vec<vk::VertexInputBindingDescription>,
    pub vertex_attributes: Vec<vk::VertexInputAttributeDescription>,
    pub push_constant_size: u32,
    pub depth_test: bool,
    pub depth_write: bool,
    pub blend: bool,
    pub cull_back: bool,
    pub descriptor_layouts: Vec<vk::DescriptorSetLayout>,
}

pub fn create_pipeline(
    device: &Device,
    config: &PipelineConfig,
    render_pass: vk::RenderPass,
) -> Result<(vk::Pipeline, vk::PipelineLayout), VkError> {
    let vs = create_shader_module(device, config.vs_bytes)?;
    let fs = create_shader_module(device, config.fs_bytes)?;

    let stages = [
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vs)
            .name(c"main"),
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(fs)
            .name(c"main"),
    ];

    let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
        .vertex_binding_descriptions(&config.vertex_bindings)
        .vertex_attribute_descriptions(&config.vertex_attributes);

    let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

    // Viewport and scissor are set while recording, so a resize does not
    // require new pipelines.
    let viewport_state = vk::PipelineViewportStateCreateInfo::default()
        .viewport_count(1)
        .scissor_count(1);
    let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
    let dynamic_state =
        vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

    let raster = vk::PipelineRasterizationStateCreateInfo::default()
        .polygon_mode(vk::PolygonMode::FILL)
        .line_width(1.0)
        .cull_mode(if config.cull_back {
            vk::CullModeFlags::BACK
        } else {
            vk::CullModeFlags::NONE
        })
        // Faces are wound counter-clockwise seen from outside the block. The
        // projection flips Y, which keeps that winding counter-clockwise on
        // screen.
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE);

    let multisample = vk::PipelineMultisampleStateCreateInfo::default()
        .rasterization_samples(device.msaa_samples);

    let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
        .depth_test_enable(config.depth_test)
        .depth_write_enable(config.depth_write)
        .depth_compare_op(vk::CompareOp::LESS);

    let blend_attachment = vk::PipelineColorBlendAttachmentState::default()
        .color_write_mask(vk::ColorComponentFlags::RGBA)
        .blend_enable(config.blend)
        .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
        .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .color_blend_op(vk::BlendOp::ADD)
        .src_alpha_blend_factor(vk::BlendFactor::ONE)
        .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
        .alpha_blend_op(vk::BlendOp::ADD);

    let blend_attachments = [blend_attachment];
    let color_blend =
        vk::PipelineColorBlendStateCreateInfo::default().attachments(&blend_attachments);

    let push_ranges = [vk::PushConstantRange::default()
        .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
        .offset(0)
        .size(config.push_constant_size)];

    let layout_info = vk::PipelineLayoutCreateInfo::default()
        .set_layouts(&config.descriptor_layouts)
        .push_constant_ranges(&push_ranges);

    let layout = unsafe { device.device.create_pipeline_layout(&layout_info, None) }
        .map_err(|e| VkError::new("create_pipeline_layout", e))?;

    let info = vk::GraphicsPipelineCreateInfo::default()
        .stages(&stages)
        .vertex_input_state(&vertex_input)
        .input_assembly_state(&input_assembly)
        .viewport_state(&viewport_state)
        .rasterization_state(&raster)
        .multisample_state(&multisample)
        .depth_stencil_state(&depth_stencil)
        .color_blend_state(&color_blend)
        .dynamic_state(&dynamic_state)
        .layout(layout)
        .render_pass(render_pass)
        .subpass(0);

    let pipeline = unsafe {
        device
            .device
            .create_graphics_pipelines(vk::PipelineCache::null(), &[info], None)
    }
    .map_err(|(_, e)| VkError::new("create_graphics_pipelines", e))?[0];

    unsafe {
        device.device.destroy_shader_module(vs, None);
        device.device.destroy_shader_module(fs, None);
    }

    Ok((pipeline, layout))
}

fn create_shader_module(device: &Device, bytes: &[u8]) -> Result<vk::ShaderModule, VkError> {
    let mut code = vec![0u32; bytes.len() / 4];
    for (i, chunk) in bytes.chunks_exact(4).enumerate() {
        code[i] = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }

    let info = vk::ShaderModuleCreateInfo::default().code(&code);
    unsafe { device.device.create_shader_module(&info, None) }
        .map_err(|e| VkError::new("create_shader_module", e))
}
