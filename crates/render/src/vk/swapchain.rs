//! Swapchain, render targets, and per-frame synchronization.

use ash::vk;

use super::device::{Device, MAX_FRAMES_IN_FLIGHT};
use super::error::VkError;

pub struct Swapchain {
    pub handle: vk::SwapchainKHR,
    pub extent: vk::Extent2D,
    pub framebuffers: Vec<vk::Framebuffer>,
    /// Signalled when rendering into image `i` finishes; present waits on it.
    /// There is one per image because the presentation engine may still hold
    /// a semaphore after the frame that signalled it has completed.
    pub render_finished: Vec<vk::Semaphore>,
    image_views: Vec<vk::ImageView>,
    depth: Attachment,
    msaa: Option<Attachment>,
}

pub struct FrameSync {
    pub image_available: vk::Semaphore,
    pub in_flight: vk::Fence,
}

impl Swapchain {
    pub fn new(device: &Device, width: u32, height: u32) -> Result<Self, VkError> {
        let caps = surface_capabilities(device)?;

        let present_modes = unsafe {
            device
                .surface_fn
                .get_physical_device_surface_present_modes(device.physical, device.surface)
        }
        .map_err(|e| VkError::new("present modes", e))?;

        // FIFO is the only present mode every driver must support.
        let present_mode = if present_modes.contains(&vk::PresentModeKHR::MAILBOX) {
            vk::PresentModeKHR::MAILBOX
        } else {
            vk::PresentModeKHR::FIFO
        };

        // OPAQUE is the normal choice. Some Android surfaces only offer INHERIT.
        let composite_alpha = [
            vk::CompositeAlphaFlagsKHR::OPAQUE,
            vk::CompositeAlphaFlagsKHR::INHERIT,
            vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED,
            vk::CompositeAlphaFlagsKHR::POST_MULTIPLIED,
        ]
        .into_iter()
        .find(|&mode| caps.supported_composite_alpha.contains(mode))
        .unwrap_or(vk::CompositeAlphaFlagsKHR::OPAQUE);

        let extent = choose_extent(&caps, width, height);

        let mut image_count = caps.min_image_count + 1;
        if caps.max_image_count > 0 {
            image_count = image_count.min(caps.max_image_count);
        }

        let format = device.surface_format;
        let info = vk::SwapchainCreateInfoKHR::default()
            .surface(device.surface)
            .min_image_count(image_count)
            .image_format(format.format)
            .image_color_space(format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(caps.current_transform)
            .composite_alpha(composite_alpha)
            .present_mode(present_mode)
            .clipped(true);

        let handle = unsafe { device.swapchain_fn.create_swapchain(&info, None) }
            .map_err(|e| VkError::new("create_swapchain", e))?;

        let images = unsafe { device.swapchain_fn.get_swapchain_images(handle) }
            .map_err(|e| VkError::new("get_swapchain_images", e))?;

        let image_views = images
            .iter()
            .map(|&image| {
                create_image_view(device, image, format.format, vk::ImageAspectFlags::COLOR)
            })
            .collect::<Result<Vec<_>, _>>()?;

        let semaphore_info = vk::SemaphoreCreateInfo::default();
        let render_finished = images
            .iter()
            .map(|_| unsafe { device.device.create_semaphore(&semaphore_info, None) })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| VkError::new("create_semaphore", e))?;

        let depth = Attachment::new(
            device,
            extent,
            device.depth_format,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            vk::ImageAspectFlags::DEPTH,
        )?;

        let msaa = if device.msaa_samples != vk::SampleCountFlags::TYPE_1 {
            Some(Attachment::new(
                device,
                extent,
                format.format,
                vk::ImageUsageFlags::COLOR_ATTACHMENT,
                vk::ImageAspectFlags::COLOR,
            )?)
        } else {
            None
        };

        let mut framebuffers = Vec::with_capacity(image_views.len());
        for &view in &image_views {
            // Same order as the render pass: colour, resolve (MSAA only), depth.
            let attachments = match &msaa {
                Some(target) => vec![target.view, view, depth.view],
                None => vec![view, depth.view],
            };

            let info = vk::FramebufferCreateInfo::default()
                .render_pass(device.render_pass)
                .attachments(&attachments)
                .width(extent.width)
                .height(extent.height)
                .layers(1);

            let framebuffer = unsafe { device.device.create_framebuffer(&info, None) }
                .map_err(|e| VkError::new("create_framebuffer", e))?;
            framebuffers.push(framebuffer);
        }

        Ok(Self {
            handle,
            extent,
            framebuffers,
            render_finished,
            image_views,
            depth,
            msaa,
        })
    }

    /// Rebuild everything that depends on the surface size.
    pub fn recreate(&mut self, device: &Device, width: u32, height: u32) -> Result<(), VkError> {
        // A minimised window can report a 0x0 surface, which no swapchain can
        // match. Keep the old one until the window has a size again.
        let caps = surface_capabilities(device)?;
        if caps.current_extent.width == 0 || caps.current_extent.height == 0 {
            return Ok(());
        }

        unsafe { device.device.device_wait_idle() }
            .map_err(|e| VkError::new("device_wait_idle", e))?;

        unsafe { self.destroy(device) };
        *self = Self::new(device, width, height)?;
        Ok(())
    }

    /// Destroyed handles are reset to null, so calling this twice is safe.
    ///
    /// # Safety
    /// Requires idle device.
    pub unsafe fn destroy(&mut self, device: &Device) {
        for framebuffer in self.framebuffers.drain(..) {
            device.device.destroy_framebuffer(framebuffer, None);
        }
        for view in self.image_views.drain(..) {
            device.device.destroy_image_view(view, None);
        }
        for semaphore in self.render_finished.drain(..) {
            device.device.destroy_semaphore(semaphore, None);
        }
        if let Some(mut msaa) = self.msaa.take() {
            msaa.destroy(device);
        }
        self.depth.destroy(device);
        device
            .swapchain_fn
            .destroy_swapchain(std::mem::take(&mut self.handle), None);
    }
}

impl FrameSync {
    pub fn new(device: &Device) -> Result<Vec<Self>, VkError> {
        let semaphore_info = vk::SemaphoreCreateInfo::default();
        let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);

        let mut frames = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
        for _ in 0..MAX_FRAMES_IN_FLIGHT {
            let image_available = unsafe { device.device.create_semaphore(&semaphore_info, None) }
                .map_err(|e| VkError::new("create_semaphore", e))?;
            let in_flight = unsafe { device.device.create_fence(&fence_info, None) }
                .map_err(|e| VkError::new("create_fence", e))?;
            frames.push(Self {
                image_available,
                in_flight,
            });
        }
        Ok(frames)
    }

    /// # Safety
    /// Requires idle device.
    pub unsafe fn destroy_all(device: &Device, frames: &[Self]) {
        for frame in frames {
            device.device.destroy_semaphore(frame.image_available, None);
            device.device.destroy_fence(frame.in_flight, None);
        }
    }
}

/// An image the render pass draws into but never presents: the depth buffer
/// or the multisampled colour target.
struct Attachment {
    image: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
}

impl Attachment {
    fn new(
        device: &Device,
        extent: vk::Extent2D,
        format: vk::Format,
        usage: vk::ImageUsageFlags,
        aspect: vk::ImageAspectFlags,
    ) -> Result<Self, VkError> {
        let info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .extent(vk::Extent3D {
                width: extent.width.max(1),
                height: extent.height.max(1),
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(device.msaa_samples)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);

        let image = unsafe { device.device.create_image(&info, None) }
            .map_err(|e| VkError::new("create_image", e))?;

        let requirements = unsafe { device.device.get_image_memory_requirements(image) };
        let memory_type = find_memory_type(
            device,
            requirements.memory_type_bits,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;

        let alloc = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(memory_type);

        let memory = unsafe { device.device.allocate_memory(&alloc, None) }
            .map_err(|e| VkError::new("allocate_memory", e))?;

        unsafe { device.device.bind_image_memory(image, memory, 0) }
            .map_err(|e| VkError::new("bind_image_memory", e))?;

        let view = create_image_view(device, image, format, aspect)?;

        Ok(Self {
            image,
            memory,
            view,
        })
    }

    unsafe fn destroy(&mut self, device: &Device) {
        device
            .device
            .destroy_image_view(std::mem::take(&mut self.view), None);
        device
            .device
            .destroy_image(std::mem::take(&mut self.image), None);
        device
            .device
            .free_memory(std::mem::take(&mut self.memory), None);
    }
}

fn surface_capabilities(device: &Device) -> Result<vk::SurfaceCapabilitiesKHR, VkError> {
    unsafe {
        device
            .surface_fn
            .get_physical_device_surface_capabilities(device.physical, device.surface)
    }
    .map_err(|e| VkError::new("surface capabilities", e))
}

fn choose_extent(caps: &vk::SurfaceCapabilitiesKHR, width: u32, height: u32) -> vk::Extent2D {
    if caps.current_extent.width != u32::MAX {
        caps.current_extent
    } else {
        vk::Extent2D {
            width: width.clamp(caps.min_image_extent.width, caps.max_image_extent.width),
            height: height.clamp(caps.min_image_extent.height, caps.max_image_extent.height),
        }
    }
}

pub fn create_image_view(
    device: &Device,
    image: vk::Image,
    format: vk::Format,
    aspect: vk::ImageAspectFlags,
) -> Result<vk::ImageView, VkError> {
    let info = vk::ImageViewCreateInfo::default()
        .image(image)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(format)
        .subresource_range(
            vk::ImageSubresourceRange::default()
                .aspect_mask(aspect)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(1),
        );

    unsafe { device.device.create_image_view(&info, None) }
        .map_err(|e| VkError::new("create_image_view", e))
}

pub fn find_memory_type(
    device: &Device,
    type_bits: u32,
    props: vk::MemoryPropertyFlags,
) -> Result<u32, VkError> {
    let mem_props = unsafe {
        device
            .instance
            .get_physical_device_memory_properties(device.physical)
    };
    for i in 0..mem_props.memory_type_count {
        if type_bits & (1 << i) != 0
            && mem_props.memory_types[i as usize]
                .property_flags
                .contains(props)
        {
            return Ok(i);
        }
    }
    Err(VkError::msg("no memory type with the required properties"))
}
