//! GPU buffers, textures, and pipeline creation helpers.

use ash::vk;

use super::device::Device;
use super::error::VkError;
use super::swapchain::find_memory_type;

/// Create a buffer and bind fresh memory with `properties` to it.
fn allocate_buffer(
    device: &Device,
    size: u64,
    usage: vk::BufferUsageFlags,
    properties: vk::MemoryPropertyFlags,
) -> Result<(vk::Buffer, vk::DeviceMemory, u64), VkError> {
    let info = vk::BufferCreateInfo::default()
        .size(size)
        .usage(usage)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    let buffer = unsafe { device.device.create_buffer(&info, None) }
        .map_err(|e| VkError::new("create_buffer", e))?;

    let req = unsafe { device.device.get_buffer_memory_requirements(buffer) };
    let bound =
        find_memory_type(device, req.memory_type_bits, properties).and_then(|memory_type| {
            let alloc = vk::MemoryAllocateInfo::default()
                .allocation_size(req.size)
                .memory_type_index(memory_type);
            let memory = unsafe { device.device.allocate_memory(&alloc, None) }
                .map_err(|e| VkError::new("allocate_memory", e))?;
            if let Err(e) = unsafe { device.device.bind_buffer_memory(buffer, memory, 0) } {
                unsafe { device.device.free_memory(memory, None) };
                return Err(VkError::new("bind_buffer_memory", e));
            }
            Ok(memory)
        });

    match bound {
        Ok(memory) => Ok((buffer, memory, req.size)),
        Err(e) => {
            unsafe { device.device.destroy_buffer(buffer, None) };
            Err(e)
        }
    }
}

/// A host-visible buffer that stays mapped. Holds per-frame vertex data, and
/// doubles as the staging buffer for uploads.
pub struct DynamicBuffer {
    pub buffer: vk::Buffer,
    pub memory: vk::DeviceMemory,
    pub size: u64,
    mapped: *mut u8,
}

impl DynamicBuffer {
    pub fn new(device: &Device, size: u64, usage: vk::BufferUsageFlags) -> Result<Self, VkError> {
        let (buffer, memory, size) = allocate_buffer(
            device,
            size,
            usage,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;
        let mapped = unsafe {
            device
                .device
                .map_memory(memory, 0, size, vk::MemoryMapFlags::empty())
        }
        .map_err(|e| VkError::new("map_memory", e))? as *mut u8;

        Ok(Self {
            buffer,
            memory,
            size,
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
    /// The GPU must be done with the buffer.
    pub unsafe fn destroy(&mut self, device: &Device) {
        device.device.unmap_memory(self.memory);
        device.device.destroy_buffer(self.buffer, None);
        device.device.free_memory(self.memory, None);
    }
}

/// A buffer in device-local memory, filled by copying from a staging buffer.
pub struct GpuBuffer {
    pub buffer: vk::Buffer,
    pub memory: vk::DeviceMemory,
}

impl GpuBuffer {
    pub fn new(device: &Device, size: u64, usage: vk::BufferUsageFlags) -> Result<Self, VkError> {
        let (buffer, memory, _) = allocate_buffer(
            device,
            size,
            usage | vk::BufferUsageFlags::TRANSFER_DST,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;
        Ok(Self { buffer, memory })
    }

    /// # Safety
    /// The GPU must be done with the buffer.
    pub unsafe fn destroy(&mut self, device: &Device) {
        device.device.destroy_buffer(self.buffer, None);
        device.device.free_memory(self.memory, None);
    }
}

/// Pixels for a texture: every array layer of mip level 0, then every layer
/// of level 1, and so on, each one tightly packed.
pub struct ImageData<'a> {
    pub format: vk::Format,
    pub bytes_per_texel: u32,
    pub width: u32,
    pub height: u32,
    pub layers: u32,
    pub levels: u32,
    pub view_type: vk::ImageViewType,
    pub pixels: &'a [u8],
}

impl ImageData<'_> {
    fn level_size(&self, level: u32) -> u64 {
        let w = (self.width >> level).max(1) as u64;
        let h = (self.height >> level).max(1) as u64;
        w * h * self.layers as u64 * self.bytes_per_texel as u64
    }
}

/// A sampled image, its view, and its sampler.
pub struct Texture {
    pub image: vk::Image,
    pub memory: vk::DeviceMemory,
    pub view: vk::ImageView,
    pub sampler: vk::Sampler,
}

impl Texture {
    /// Upload `data` and leave the image ready for sampling in fragment
    /// shaders.
    ///
    /// The pixels go through a staging buffer into an optimally tiled image.
    /// Vulkan 1.0 guarantees sampling support for the formats used here with
    /// optimal tiling only; linear tiling support is optional.
    pub fn new(
        device: &Device,
        data: &ImageData,
        sampler_info: &vk::SamplerCreateInfo,
    ) -> Result<Self, VkError> {
        let info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(data.format)
            .extent(vk::Extent3D {
                width: data.width,
                height: data.height,
                depth: 1,
            })
            .mip_levels(data.levels)
            .array_layers(data.layers)
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
            data.pixels.len() as u64,
            vk::BufferUsageFlags::TRANSFER_SRC,
        )?;
        staging.write(data.pixels);
        let uploaded = unsafe { upload(device, staging.buffer, image, data) };
        unsafe { staging.destroy(device) };
        uploaded?;

        let range = vk::ImageSubresourceRange::default()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .base_mip_level(0)
            .level_count(data.levels)
            .base_array_layer(0)
            .layer_count(data.layers);
        let view_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(data.view_type)
            .format(data.format)
            .subresource_range(range);
        let view = unsafe { device.device.create_image_view(&view_info, None) }
            .map_err(|e| VkError::new("create_image_view", e))?;

        let sampler = unsafe { device.device.create_sampler(sampler_info, None) }
            .map_err(|e| VkError::new("create_sampler", e))?;

        Ok(Self {
            image,
            memory,
            view,
            sampler,
        })
    }

    /// # Safety
    /// The GPU must be done with the texture.
    pub unsafe fn destroy(&mut self, device: &Device) {
        device.device.destroy_sampler(self.sampler, None);
        device.device.destroy_image_view(self.view, None);
        device.device.destroy_image(self.image, None);
        device.device.free_memory(self.memory, None);
    }
}

/// Copy every mip level of `data` from `buffer` into `image` and move the
/// image to SHADER_READ_ONLY_OPTIMAL. Blocks until the GPU is done.
///
/// # Safety
/// `image` must be freshly created to match `data`, with TRANSFER_DST usage.
unsafe fn upload(
    device: &Device,
    buffer: vk::Buffer,
    image: vk::Image,
    data: &ImageData,
) -> Result<(), VkError> {
    let range = vk::ImageSubresourceRange::default()
        .aspect_mask(vk::ImageAspectFlags::COLOR)
        .base_mip_level(0)
        .level_count(data.levels)
        .base_array_layer(0)
        .layer_count(data.layers);

    let mut offset = 0;
    let regions: Vec<vk::BufferImageCopy> = (0..data.levels)
        .map(|level| {
            let region = vk::BufferImageCopy::default()
                .buffer_offset(offset)
                .image_subresource(
                    vk::ImageSubresourceLayers::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .mip_level(level)
                        .base_array_layer(0)
                        .layer_count(data.layers),
                )
                .image_extent(vk::Extent3D {
                    width: (data.width >> level).max(1),
                    height: (data.height >> level).max(1),
                    depth: 1,
                });
            offset += data.level_size(level);
            region
        })
        .collect();

    submit_and_wait(device, |cmd| {
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

        // Buffer row length 0 means the rows are tightly packed.
        device.device.cmd_copy_buffer_to_image(
            cmd,
            buffer,
            image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &regions,
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
    })
}

/// Record commands with `record` into a one-off command buffer, submit it,
/// and wait for the GPU to finish.
unsafe fn submit_and_wait(
    device: &Device,
    record: impl FnOnce(vk::CommandBuffer),
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
    record(cmd);
    device
        .device
        .end_command_buffer(cmd)
        .map_err(|e| VkError::new("end_command_buffer", e))?;

    let fence = device
        .device
        .create_fence(&vk::FenceCreateInfo::default(), None)
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
    /// Render depth only, for the shadow map: one sample, no colour output,
    /// and a depth bias so surfaces do not shadow themselves.
    pub depth_only: bool,
    pub descriptor_layouts: Vec<vk::DescriptorSetLayout>,
    /// Value for specialization constant 0 in both shader stages, if the
    /// shaders declare one.
    pub specialization: Option<u32>,
}

pub fn create_pipeline(
    device: &Device,
    config: &PipelineConfig,
    render_pass: vk::RenderPass,
) -> Result<(vk::Pipeline, vk::PipelineLayout), VkError> {
    let vs = create_shader_module(device, config.vs_bytes)?;
    let fs = create_shader_module(device, config.fs_bytes)?;

    let spec_value = config.specialization.unwrap_or(0).to_ne_bytes();
    let spec_entries = [vk::SpecializationMapEntry::default()
        .constant_id(0)
        .offset(0)
        .size(4)];
    let spec_info = vk::SpecializationInfo::default()
        .map_entries(&spec_entries)
        .data(&spec_value);

    let mut stages = [
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vs)
            .name(c"main"),
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(fs)
            .name(c"main"),
    ];
    if config.specialization.is_some() {
        for stage in &mut stages {
            *stage = stage.specialization_info(&spec_info);
        }
    }

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
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
        .depth_bias_enable(config.depth_only)
        .depth_bias_constant_factor(1.5)
        .depth_bias_slope_factor(2.0);

    let samples = if config.depth_only {
        vk::SampleCountFlags::TYPE_1
    } else {
        device.msaa_samples
    };
    let multisample =
        vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(samples);

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
        vk::PipelineColorBlendStateCreateInfo::default().attachments(if config.depth_only {
            &[]
        } else {
            &blend_attachments
        });

    let push_range = [vk::PushConstantRange::default()
        .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
        .offset(0)
        .size(config.push_constant_size)];
    let push_ranges: &[vk::PushConstantRange] = if config.push_constant_size == 0 {
        &[]
    } else {
        &push_range
    };

    let layout_info = vk::PipelineLayoutCreateInfo::default()
        .set_layouts(&config.descriptor_layouts)
        .push_constant_ranges(push_ranges);

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
