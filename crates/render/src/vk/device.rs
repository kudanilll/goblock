//! Vulkan setup: instance, surface, physical and logical device, and the
//! render pass.
//!
//! Only Vulkan 1.0 core plus `VK_KHR_surface` and `VK_KHR_swapchain` are
//! required. Anything beyond that (validation, 4x MSAA, mailbox present) is
//! probed first and switched off when the device lacks it.

use std::ffi::{c_char, CStr};

use ash::vk;

use super::error::VkError;

pub const VALIDATION_LAYER: &CStr = c"VK_LAYER_KHRONOS_validation";
const DEVICE_EXTENSIONS: [&CStr; 1] = [ash::khr::swapchain::NAME];

/// Number of frames the CPU may run ahead of the GPU.
pub const MAX_FRAMES_IN_FLIGHT: usize = 2;

pub struct Device {
    pub entry: ash::Entry,
    pub instance: ash::Instance,
    pub surface_fn: ash::khr::surface::Instance,
    pub surface: vk::SurfaceKHR,
    pub physical: vk::PhysicalDevice,
    pub device: ash::Device,
    pub swapchain_fn: ash::khr::swapchain::Device,
    pub queue: vk::Queue,
    pub queue_family: u32,
    pub render_pass: vk::RenderPass,
    pub surface_format: vk::SurfaceFormatKHR,
    pub depth_format: vk::Format,
    pub msaa_samples: vk::SampleCountFlags,
    pub properties: vk::PhysicalDeviceProperties,
    debug_messenger: Option<(ash::ext::debug_utils::Instance, vk::DebugUtilsMessengerEXT)>,
}

impl Device {
    /// # Safety
    /// `raw_handle` must stay valid for the lifetime of the surface.
    pub unsafe fn new(
        entry: ash::Entry,
        raw_handle: raw_window_handle::RawWindowHandle,
        raw_display: raw_window_handle::RawDisplayHandle,
        enable_validation: bool,
    ) -> Result<Self, VkError> {
        let validation = enable_validation && validation_available(&entry);
        if enable_validation && !validation {
            eprintln!("goblock: validation layer not found, running without it");
        }

        let instance = create_instance(&entry, validation, raw_display)?;

        let surface_fn = ash::khr::surface::Instance::new(&entry, &instance);
        let surface = ash_window::create_surface(&entry, &instance, raw_display, raw_handle, None)
            .map_err(|e| VkError::new("create_surface", e))?;

        let physical = pick_physical_device(&instance, &surface_fn, surface)?;
        let properties = instance.get_physical_device_properties(physical);
        let queue_family = pick_queue_family(&instance, &surface_fn, surface, physical)?;

        let device = create_logical_device(&instance, physical, queue_family)?;
        let swapchain_fn = ash::khr::swapchain::Device::new(&instance, &device);
        let queue = device.get_device_queue(queue_family, 0);

        let surface_format = pick_surface_format(&surface_fn, physical, surface)?;
        let depth_format = pick_depth_format(&instance, physical);
        let msaa_samples = pick_sample_count(&properties);

        let debug_messenger = if validation {
            Some(create_debug_messenger(&entry, &instance)?)
        } else {
            None
        };

        let render_pass =
            create_render_pass(&device, surface_format.format, depth_format, msaa_samples)?;

        Ok(Self {
            entry,
            instance,
            surface_fn,
            surface,
            physical,
            device,
            swapchain_fn,
            queue,
            queue_family,
            render_pass,
            surface_format,
            depth_format,
            msaa_samples,
            properties,
            debug_messenger,
        })
    }

    /// # Safety
    /// Requires the device to be idle.
    pub unsafe fn destroy(&mut self) {
        if let Some((loader, messenger)) = self.debug_messenger.take() {
            loader.destroy_debug_utils_messenger(messenger, None);
        }
        self.device.destroy_render_pass(self.render_pass, None);
        self.surface_fn.destroy_surface(self.surface, None);
        self.device.destroy_device(None);
        self.instance.destroy_instance(None);
    }
}

/// Validation needs the Khronos layer and `VK_EXT_debug_utils`. Machines
/// without the Vulkan SDK, and loaders older than 1.1, lack one or both.
unsafe fn validation_available(entry: &ash::Entry) -> bool {
    let has_layer = entry
        .enumerate_instance_layer_properties()
        .unwrap_or_default()
        .iter()
        .any(|l| l.layer_name_as_c_str() == Ok(VALIDATION_LAYER));

    let has_debug_utils = [None, Some(VALIDATION_LAYER)].into_iter().any(|layer| {
        entry
            .enumerate_instance_extension_properties(layer)
            .unwrap_or_default()
            .iter()
            .any(|e| e.extension_name_as_c_str() == Ok(ash::ext::debug_utils::NAME))
    });

    has_layer && has_debug_utils
}

unsafe fn create_instance(
    entry: &ash::Entry,
    validation: bool,
    raw_display: raw_window_handle::RawDisplayHandle,
) -> Result<ash::Instance, VkError> {
    // A 1.0 loader rejects any higher version with VK_ERROR_INCOMPATIBLE_DRIVER,
    // and nothing in the renderer needs more than 1.0.
    let app_info = vk::ApplicationInfo::default()
        .application_name(c"goblock")
        .application_version(vk::make_api_version(0, 0, 1, 0))
        .engine_name(c"goblock")
        .engine_version(vk::make_api_version(0, 0, 1, 0))
        .api_version(vk::API_VERSION_1_0);

    let layers: Vec<*const c_char> = if validation {
        vec![VALIDATION_LAYER.as_ptr()]
    } else {
        vec![]
    };

    let mut extensions = ash_window::enumerate_required_extensions(raw_display)
        .map_err(|_| VkError::msg("could not enumerate required surface extensions"))?
        .to_vec();
    if validation {
        extensions.push(ash::ext::debug_utils::NAME.as_ptr());
    }

    let info = vk::InstanceCreateInfo::default()
        .application_info(&app_info)
        .enabled_layer_names(&layers)
        .enabled_extension_names(&extensions);

    entry
        .create_instance(&info, None)
        .map_err(|e| VkError::new("create_instance", e))
}

unsafe fn create_debug_messenger(
    entry: &ash::Entry,
    instance: &ash::Instance,
) -> Result<(ash::ext::debug_utils::Instance, vk::DebugUtilsMessengerEXT), VkError> {
    let loader = ash::ext::debug_utils::Instance::new(entry, instance);

    let info = vk::DebugUtilsMessengerCreateInfoEXT::default()
        .message_severity(
            vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING,
        )
        .message_type(
            vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
        )
        .pfn_user_callback(Some(vulkan_debug_callback));

    let messenger = loader
        .create_debug_utils_messenger(&info, None)
        .map_err(|e| VkError::new("create_debug_utils_messenger", e))?;

    Ok((loader, messenger))
}

unsafe extern "system" fn vulkan_debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    types: vk::DebugUtilsMessageTypeFlagsEXT,
    data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _user: *mut std::ffi::c_void,
) -> vk::Bool32 {
    let data = &*data;
    let message = CStr::from_ptr(data.p_message).to_string_lossy();
    let level = if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
        "error"
    } else {
        "warn"
    };
    eprintln!("[vulkan {level} {types:?}] {message}");
    vk::FALSE
}

/// Pick the best device that can draw to `surface`. Discrete GPUs win, but
/// virtual and software devices still count, so the game also starts in a
/// VM or on a CPU implementation.
fn pick_physical_device(
    instance: &ash::Instance,
    surface_fn: &ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
) -> Result<vk::PhysicalDevice, VkError> {
    let devices = unsafe { instance.enumerate_physical_devices() }
        .map_err(|e| VkError::new("enumerate_physical_devices", e))?;

    devices
        .into_iter()
        .filter(|&device| pick_queue_family(instance, surface_fn, surface, device).is_ok())
        .max_by_key(|&device| {
            match unsafe { instance.get_physical_device_properties(device) }.device_type {
                vk::PhysicalDeviceType::DISCRETE_GPU => 3,
                vk::PhysicalDeviceType::INTEGRATED_GPU => 2,
                vk::PhysicalDeviceType::VIRTUAL_GPU => 1,
                _ => 0,
            }
        })
        .ok_or_else(|| {
            VkError::msg("no suitable Vulkan device with a graphics queue and present support")
        })
}

fn pick_queue_family(
    instance: &ash::Instance,
    surface_fn: &ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
    device: vk::PhysicalDevice,
) -> Result<u32, VkError> {
    let families = unsafe { instance.get_physical_device_queue_family_properties(device) };
    for (index, family) in families.iter().enumerate() {
        let graphics = family.queue_flags.contains(vk::QueueFlags::GRAPHICS);
        let present = unsafe {
            surface_fn.get_physical_device_surface_support(device, index as u32, surface)
        }
        .unwrap_or(false);
        if graphics && present {
            return Ok(index as u32);
        }
    }
    Err(VkError::msg(
        "no queue family supports both graphics and present",
    ))
}

unsafe fn create_logical_device(
    instance: &ash::Instance,
    physical: vk::PhysicalDevice,
    queue_family: u32,
) -> Result<ash::Device, VkError> {
    let priorities = [1.0f32];
    let queue_info = vk::DeviceQueueCreateInfo::default()
        .queue_family_index(queue_family)
        .queue_priorities(&priorities);

    let queues = [queue_info];
    let extension_ptrs: Vec<*const c_char> = DEVICE_EXTENSIONS.iter().map(|e| e.as_ptr()).collect();

    // No optional features are enabled, so any 1.0 device accepts this.
    let info = vk::DeviceCreateInfo::default()
        .queue_create_infos(&queues)
        .enabled_extension_names(&extension_ptrs);

    instance
        .create_device(physical, &info, None)
        .map_err(|e| VkError::new("create_device", e))
}

/// Prefer an sRGB format so the shaders' linear output is gamma corrected.
/// Desktop drivers usually offer BGRA and Android drivers RGBA, so both are
/// accepted. Otherwise take whatever the surface lists first.
fn pick_surface_format(
    surface_fn: &ash::khr::surface::Instance,
    physical: vk::PhysicalDevice,
    surface: vk::SurfaceKHR,
) -> Result<vk::SurfaceFormatKHR, VkError> {
    let formats = unsafe { surface_fn.get_physical_device_surface_formats(physical, surface) }
        .map_err(|e| VkError::new("surface formats", e))?;

    // Early drivers report a single UNDEFINED entry when any format works.
    if let [only] = formats.as_slice() {
        if only.format == vk::Format::UNDEFINED {
            return Ok(vk::SurfaceFormatKHR {
                format: vk::Format::B8G8R8A8_SRGB,
                color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
            });
        }
    }

    formats
        .iter()
        .find(|f| {
            matches!(
                f.format,
                vk::Format::B8G8R8A8_SRGB | vk::Format::R8G8B8A8_SRGB
            ) && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
        })
        .or_else(|| formats.first())
        .copied()
        .ok_or_else(|| VkError::msg("surface reports no formats"))
}

/// Every Vulkan 1.0 device must support D16_UNORM as a depth attachment, so
/// it is the fallback when none of the more precise formats are available.
fn pick_depth_format(instance: &ash::Instance, physical: vk::PhysicalDevice) -> vk::Format {
    [
        vk::Format::D32_SFLOAT,
        vk::Format::X8_D24_UNORM_PACK32,
        vk::Format::D24_UNORM_S8_UINT,
        vk::Format::D32_SFLOAT_S8_UINT,
    ]
    .into_iter()
    .find(|&format| {
        unsafe { instance.get_physical_device_format_properties(physical, format) }
            .optimal_tiling_features
            .contains(vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT)
    })
    .unwrap_or(vk::Format::D16_UNORM)
}

/// 4x MSAA when the device supports it for both colour and depth, otherwise
/// no multisampling.
fn pick_sample_count(props: &vk::PhysicalDeviceProperties) -> vk::SampleCountFlags {
    let counts =
        props.limits.framebuffer_color_sample_counts & props.limits.framebuffer_depth_sample_counts;

    if counts.contains(vk::SampleCountFlags::TYPE_4) {
        vk::SampleCountFlags::TYPE_4
    } else {
        vk::SampleCountFlags::TYPE_1
    }
}

unsafe fn create_render_pass(
    device: &ash::Device,
    color_format: vk::Format,
    depth_format: vk::Format,
    samples: vk::SampleCountFlags,
) -> Result<vk::RenderPass, VkError> {
    let msaa = samples != vk::SampleCountFlags::TYPE_1;

    // Attachment order is colour, then the resolve target when MSAA is on,
    // then depth. Framebuffers and clear values use the same order.
    let mut attachments = vec![vk::AttachmentDescription::default()
        .format(color_format)
        .samples(samples)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        // With MSAA only the resolved image is kept.
        .store_op(if msaa {
            vk::AttachmentStoreOp::DONT_CARE
        } else {
            vk::AttachmentStoreOp::STORE
        })
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(if msaa {
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL
        } else {
            vk::ImageLayout::PRESENT_SRC_KHR
        })];

    if msaa {
        attachments.push(
            vk::AttachmentDescription::default()
                .format(color_format)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::DONT_CARE)
                .store_op(vk::AttachmentStoreOp::STORE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::PRESENT_SRC_KHR),
        );
    }

    let depth_ref = vk::AttachmentReference {
        attachment: attachments.len() as u32,
        layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
    };
    attachments.push(
        vk::AttachmentDescription::default()
            .format(depth_format)
            .samples(samples)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL),
    );

    let color_refs = [vk::AttachmentReference {
        attachment: 0,
        layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
    }];
    let resolve_refs = [vk::AttachmentReference {
        attachment: 1,
        layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
    }];

    let mut subpass = vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&color_refs)
        .depth_stencil_attachment(&depth_ref);
    if msaa {
        subpass = subpass.resolve_attachments(&resolve_refs);
    }

    // All frames in flight share one depth image and one MSAA image, so this
    // frame's clear has to wait for the previous frame's writes to finish.
    let dependency = vk::SubpassDependency::default()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
        )
        .src_access_mask(
            vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
        )
        .dst_stage_mask(
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
        )
        .dst_access_mask(
            vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
        );

    let dependencies = [dependency];
    let subpasses = [subpass];
    let info = vk::RenderPassCreateInfo::default()
        .attachments(&attachments)
        .subpasses(&subpasses)
        .dependencies(&dependencies);

    device
        .create_render_pass(&info, None)
        .map_err(|e| VkError::new("create_render_pass", e))
}
