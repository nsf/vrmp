// all the unsafe stuff goes here

use super::cmdpool::CmdPool;
use ash::vk;
use itertools::Itertools;
use libopenvr::Context;
use std::{ffi::CStr, sync::Arc};
use wgpu_hal::{api::Vulkan, Api, InstanceFlags};

unsafe fn is_good_device(instance: ash::Instance, pdevice: vk::PhysicalDevice) -> bool {
    let props = instance.get_physical_device_properties(pdevice);
    props.device_type == vk::PhysicalDeviceType::DISCRETE_GPU
}

pub struct LoadVulkanWGPUParams<'a, W: raw_window_handle::HasRawWindowHandle> {
    pub vr_ctx: Option<&'a libopenvr::Context>,
    pub window: &'a W,
    pub features: wgpu::Features,
    pub limits: wgpu::Limits,
    pub flags: InstanceFlags,
}

pub struct VulkanSharedTexture {
    pub gl_complete: vk::Semaphore,
    pub gl_ready: vk::Semaphore,
    pub memory: vk::DeviceMemory,
    pub gl_complete_fd: i32,
    pub gl_ready_fd: i32,
    pub gl_memory_fd: i32,

    pub memory_size: u64,
    pub width: u32,
    pub height: u32,

    pub image: vk::Image,
    pub texture: wgpu::Texture,
    pub texture_view: wgpu::TextureView,
    pub bind_group_layout: Arc<wgpu::BindGroupLayout>,
    pub bind_group: wgpu::BindGroup,
}

impl VulkanSharedTexture {
    pub unsafe fn create(
        instance: &ash::Instance,
        device: &ash::Device,
        physical_device: vk::PhysicalDevice,
        wgpu_device: &wgpu::Device,
        bind_group_layout: Arc<wgpu::BindGroupLayout>,
        w: u32,
        h: u32,
    ) -> VulkanSharedTexture {
        let mut vk_info = vk::ExportSemaphoreCreateInfo::builder()
            .handle_types(vk::ExternalSemaphoreHandleTypeFlags::OPAQUE_FD)
            .build();

        let vk_info = vk::SemaphoreCreateInfo::builder().push_next(&mut vk_info).build();

        let gl_complete = device.create_semaphore(&vk_info, None).unwrap();
        let gl_ready = device.create_semaphore(&vk_info, None).unwrap();

        let ext_semaphore = ash::extensions::khr::ExternalSemaphoreFd::new(instance, &device);
        let gl_complete_handle = ext_semaphore
            .get_semaphore_fd(
                &vk::SemaphoreGetFdInfoKHR::builder()
                    .semaphore(gl_complete)
                    .handle_type(vk::ExternalSemaphoreHandleTypeFlags::OPAQUE_FD)
                    .build(),
            )
            .unwrap();
        let gl_ready_handle = ext_semaphore
            .get_semaphore_fd(
                &vk::SemaphoreGetFdInfoKHR::builder()
                    .semaphore(gl_ready)
                    .handle_type(vk::ExternalSemaphoreHandleTypeFlags::OPAQUE_FD)
                    .build(),
            )
            .unwrap();

        let mut ext_vk_info = vk::ExternalMemoryImageCreateInfo::builder()
            .handle_types(vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD)
            .build();

        let vk_info = vk::ImageCreateInfo::builder()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::R8G8B8A8_SRGB)
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .extent(vk::Extent3D::builder().depth(1).width(w).height(h).build())
            .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)
            .tiling(vk::ImageTiling::LINEAR)
            .push_next(&mut ext_vk_info)
            .build();

        let image = device.create_image(&vk_info, None).unwrap();
        let mem_reqs = device.get_image_memory_requirements(image);
        let mem_type = get_memory_type(
            instance,
            physical_device,
            mem_reqs.memory_type_bits,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )
        .unwrap();

        let mut ext_vk_info = vk::ExportMemoryAllocateInfo::builder()
            .handle_types(vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD)
            .build();
        let mut ext_vk_info2 = vk::MemoryDedicatedAllocateInfo::builder().image(image).build();

        let vk_info = vk::MemoryAllocateInfo::builder()
            .allocation_size(mem_reqs.size)
            .memory_type_index(mem_type.heap_index)
            .push_next(&mut ext_vk_info)
            .push_next(&mut ext_vk_info2)
            .build();

        // If I understand correctly using external memory extension requires dedicated allocation. Also if I remember
        // correctly my AMD device worked without it. But let's keep it that way.
        let memory = device.allocate_memory(&vk_info, None).unwrap();
        device.bind_image_memory(image, memory, 0).unwrap();

        let ext_memory = ash::extensions::khr::ExternalMemoryFd::new(instance, &device);
        let gl_memory_handle = ext_memory
            .get_memory_fd(
                &vk::MemoryGetFdInfoKHR::builder()
                    .memory(memory)
                    .handle_type(vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD)
                    .build(),
            )
            .unwrap();

        // TODO: consider attaching destruction logic to 'drop_handle' here,
        // might free us from tracking texture usage manually
        let hal_texture = <Vulkan as Api>::Device::texture_from_raw(
            image,
            &wgpu_hal::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                mip_level_count: 1,
                sample_count: 1,
                memory_flags: wgpu_hal::MemoryFlags::empty(),
                usage: wgpu_hal::TextureUses::COLOR_TARGET | wgpu_hal::TextureUses::RESOURCE,
            },
            Some(Box::new(())),
        );
        let texture = wgpu_device.create_texture_from_hal::<Vulkan>(
            hal_texture,
            &wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                mip_level_count: 1,
                sample_count: 1,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            },
        );
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = wgpu_device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&texture_view),
            }],
        });

        VulkanSharedTexture {
            gl_complete,
            gl_ready,
            gl_complete_fd: gl_complete_handle,
            gl_ready_fd: gl_ready_handle,
            image,
            memory,
            memory_size: mem_reqs.size,
            gl_memory_fd: gl_memory_handle,
            texture,
            texture_view,
            width: w,
            height: h,
            bind_group,
            bind_group_layout,
        }
    }

    pub fn shutdown(&self, device: &ash::Device) {
        unsafe {
            device.destroy_image(self.image, None);
            device.free_memory(self.memory, None);
            device.destroy_semaphore(self.gl_complete, None);
            device.destroy_semaphore(self.gl_ready, None);
        }
    }
}

pub struct VulkanWGPU {
    pub instance: wgpu::Instance,
    pub surface: wgpu::Surface,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,

    pub ash_instance: ash::Instance,
    pub ash_device: ash::Device,
    pub vk_physical_device: vk::PhysicalDevice,
    pub vk_queue: vk::Queue,
    pub vk_queue_family_index: u32,

    pub cmd_pool: CmdPool,
}

pub struct ImageTransitionSpec {
    pub a_access_mask: vk::AccessFlags,
    pub b_access_mask: vk::AccessFlags,
    pub a_layout: vk::ImageLayout,
    pub b_layout: vk::ImageLayout,
    pub a_stage_mask: vk::PipelineStageFlags,
    pub b_stage_mask: vk::PipelineStageFlags,
}

pub fn image_transition_spec_vr() -> ImageTransitionSpec {
    ImageTransitionSpec {
        a_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        b_access_mask: vk::AccessFlags::TRANSFER_READ,
        a_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        b_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        a_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
        b_stage_mask: vk::PipelineStageFlags::TRANSFER,
    }
}

pub enum ImageTransitionDir {
    AToB,
    BToA,
}

unsafe fn get_memory_type(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    mut bits: u32,
    property_flags: vk::MemoryPropertyFlags,
) -> Option<vk::MemoryType> {
    let mprops = instance.get_physical_device_memory_properties(physical_device);
    for mt in mprops.memory_types {
        if bits & 1 == 1 {
            if mt.property_flags.contains(property_flags) {
                return Some(mt);
            }
        }
        bits >>= 1;
    }
    None
}

impl VulkanWGPU {
    pub fn shutdown(&mut self) {
        unsafe {
            self.cmd_pool.shutdown(&self.ash_device);
        }
    }

    pub unsafe fn submit_eye_textures(&mut self, vr_ctx: &Context, left_eye: &EyeData, right_eye: &EyeData) {
        self.transition_image(
            &image_transition_spec_vr(),
            left_eye.raw_handle,
            ImageTransitionDir::AToB,
        );
        self.transition_image(
            &image_transition_spec_vr(),
            right_eye.raw_handle,
            ImageTransitionDir::AToB,
        );
        {
            let texture_bounds = libopenvr::TextureBounds {
                u_min: 0.0,
                v_min: 0.0,
                u_max: 1.0,
                v_max: 1.0,
            };
            let shared_texture_data = libopenvr::VulkanTextureData {
                device: self.ash_device.handle(),
                instance: self.ash_instance.handle(),
                format: vk::Format::B8G8R8A8_SRGB,
                width: left_eye.width,
                height: left_eye.height,
                physical_device: self.vk_physical_device,
                queue: self.vk_queue,
                queue_family_index: self.vk_queue_family_index,
                sample_count: vk::SampleCountFlags::TYPE_1,
                image: vk::Image::null(),
            };

            vr_ctx.compositor.submit_vulkan(
                libopenvr::Eye::Left,
                &libopenvr::VulkanTextureData {
                    image: left_eye.raw_handle,
                    ..shared_texture_data
                },
                &texture_bounds,
            );
            vr_ctx.compositor.submit_vulkan(
                libopenvr::Eye::Right,
                &libopenvr::VulkanTextureData {
                    image: right_eye.raw_handle,
                    ..shared_texture_data
                },
                &texture_bounds,
            );
        }
        self.transition_image(
            &image_transition_spec_vr(),
            left_eye.raw_handle,
            ImageTransitionDir::BToA,
        );
        self.transition_image(
            &image_transition_spec_vr(),
            right_eye.raw_handle,
            ImageTransitionDir::BToA,
        );
    }

    pub unsafe fn transition_image(&mut self, spec: &ImageTransitionSpec, image: vk::Image, dir: ImageTransitionDir) {
        let cmd_buf = self.cmd_pool.get_buf();

        let vk_info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .build();

        self.ash_device.begin_command_buffer(cmd_buf, &vk_info).unwrap();

        let src_access_mask = spec.a_access_mask;
        let dst_access_mask = spec.b_access_mask;
        let src_layout = spec.a_layout;
        let dst_layout = spec.b_layout;
        let src_stage_mask = spec.a_stage_mask;
        let dst_stage_mask = spec.b_stage_mask;
        let subresource_range = vk::ImageSubresourceRange::builder()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .level_count(1)
            .layer_count(1)
            .build();
        match dir {
            ImageTransitionDir::AToB => {
                let src_to_dst_image_barrier = vk::ImageMemoryBarrier::builder()
                    .image(image)
                    .subresource_range(subresource_range)
                    .src_access_mask(src_access_mask)
                    .dst_access_mask(dst_access_mask)
                    .old_layout(src_layout)
                    .new_layout(dst_layout)
                    .build();

                self.ash_device.cmd_pipeline_barrier(
                    cmd_buf,
                    src_stage_mask,
                    dst_stage_mask,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[src_to_dst_image_barrier],
                );
            }
            ImageTransitionDir::BToA => {
                let dst_to_src_image_barrier = vk::ImageMemoryBarrier::builder()
                    .image(image)
                    .subresource_range(subresource_range)
                    .src_access_mask(dst_access_mask)
                    .dst_access_mask(src_access_mask)
                    .old_layout(dst_layout)
                    .new_layout(src_layout)
                    .build();

                self.ash_device.cmd_pipeline_barrier(
                    cmd_buf,
                    dst_stage_mask,
                    src_stage_mask,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[dst_to_src_image_barrier],
                );
            }
        }
        self.ash_device.end_command_buffer(cmd_buf).unwrap();

        let vk_info = vk::SubmitInfo::builder().command_buffers(&[cmd_buf]).build();
        self.ash_device
            .queue_submit(self.vk_queue, &[vk_info], vk::Fence::null())
            .unwrap();
    }

    pub unsafe fn create<'a, W: raw_window_handle::HasRawWindowHandle>(p: &LoadVulkanWGPUParams<'a, W>) -> VulkanWGPU {
        // note that "entry" is consumed by "<Vulkan as Api>::Instance::from_raw",
        // most likely wgpu keeps it around for its own needs, as well as ours
        let entry = ash::Entry::load().expect("ash entry load() failed");
        let driver_api_version = match entry.try_enumerate_instance_version().unwrap() {
            Some(version) => version,
            None => vk::API_VERSION_1_0,
        };
        let driver_api_version = if driver_api_version < vk::API_VERSION_1_1 {
            vk::API_VERSION_1_0
        } else {
            vk::HEADER_VERSION_COMPLETE
        };

        let app_info = vk::ApplicationInfo::builder()
            .application_name(CStr::from_bytes_with_nul(b"vrmp\0").unwrap())
            .application_version(1)
            .engine_name(CStr::from_bytes_with_nul(b"vrmp-wgpu-hal\0").unwrap())
            .engine_version(2)
            .api_version(driver_api_version);

        let mut instance_extensions = <Vulkan as Api>::Instance::required_extensions(&entry, p.flags).unwrap();

        add_if_doesnt_exist(
            &mut instance_extensions,
            [
                CStr::from_bytes_with_nul(b"VK_KHR_get_physical_device_properties2\0").unwrap(),
                CStr::from_bytes_with_nul(b"VK_KHR_external_semaphore_capabilities\0").unwrap(),
                CStr::from_bytes_with_nul(b"VK_KHR_external_memory_capabilities\0").unwrap(),
            ],
        );

        if let Some(vr_ctx) = p.vr_ctx {
            add_if_doesnt_exist(
                &mut instance_extensions,
                vr_ctx.compositor.get_vulkan_instance_extensions_required(),
            );
        }

        for e in instance_extensions.iter().cloned() {
            log::debug!("inst ext: {}", e.to_string_lossy());
        }

        let instance_layers = entry.enumerate_instance_layer_properties().unwrap();
        let layers = {
            let mut layers: Vec<&'static CStr> = Vec::new();
            if p.flags.contains(InstanceFlags::VALIDATION) {
                layers.push(CStr::from_bytes_with_nul(b"VK_LAYER_KHRONOS_validation\0").unwrap());
            }
            layers.retain(|&layer| {
                if instance_layers
                    .iter()
                    .any(|inst_layer| CStr::from_ptr(inst_layer.layer_name.as_ptr()) == layer)
                {
                    true
                } else {
                    log::warn!("Unable to find layer: {}", layer.to_string_lossy());
                    false
                }
            });
            layers
        };

        let ash_instance = {
            let str_pointers = layers
                .iter()
                .map(|&s| s.as_ptr())
                .chain(instance_extensions.iter().map(|&v| v.as_ptr()))
                .collect::<Vec<_>>();

            let create_info = vk::InstanceCreateInfo::builder()
                .flags(vk::InstanceCreateFlags::empty())
                .application_info(&app_info)
                .enabled_layer_names(&str_pointers[..layers.len()])
                .enabled_extension_names(&str_pointers[layers.len()..]);

            entry
                .create_instance(&create_info, None)
                .expect("ash create instance failed")
        };

        let vr_pdevice = p
            .vr_ctx
            .map(|v| v.system.get_output_device_for_vulkan(ash_instance.handle()));

        let vk_physical_device = ash_instance
            .enumerate_physical_devices()
            .unwrap()
            .iter()
            .cloned()
            .find(|&device| Some(device) == vr_pdevice || is_good_device(ash_instance.clone(), device))
            .expect("failed to find physical device required by openvr");

        let vk_queue_family_index = ash_instance
            .get_physical_device_queue_family_properties(vk_physical_device)
            .iter()
            .cloned()
            .find_position(|p| p.queue_flags.contains(vk::QueueFlags::GRAPHICS))
            .expect("failed to find graphics queue")
            .0 as u32;

        let plimits = ash_instance.get_physical_device_properties(vk_physical_device).limits;

        let hal_instance = <Vulkan as Api>::Instance::from_raw(
            entry,
            ash_instance.clone(),
            driver_api_version,
            instance_extensions,
            p.flags,
            false,
            Some(Box::new(())),
        )
        .unwrap();
        let hal_adapter = hal_instance
            .expose_adapter(vk_physical_device)
            .expect("failed exposing wgpu-hal adapater");

        let (hal_device, vk_queue, ash_device) = {
            let uab_types = wgpu_hal::UpdateAfterBindTypes::from_limits(&p.limits, &plimits);
            let mut device_extensions = hal_adapter.adapter.required_device_extensions(p.features);

            add_if_doesnt_exist(
                &mut device_extensions,
                [
                    CStr::from_bytes_with_nul(b"VK_KHR_external_memory\0").unwrap(),
                    CStr::from_bytes_with_nul(b"VK_KHR_external_memory_fd\0").unwrap(),
                    CStr::from_bytes_with_nul(b"VK_KHR_external_semaphore\0").unwrap(),
                    CStr::from_bytes_with_nul(b"VK_KHR_external_semaphore_fd\0").unwrap(),
                ],
            );

            if let Some(vr_ctx) = p.vr_ctx {
                add_if_doesnt_exist(
                    &mut device_extensions,
                    vr_ctx
                        .compositor
                        .get_vulkan_device_extensions_required(vk_physical_device),
                );
            }

            for e in device_extensions.iter().cloned() {
                log::debug!("dev ext: {}", e.to_string_lossy());
            }

            let mut enabled_phd_features =
                hal_adapter
                    .adapter
                    .physical_device_features(&device_extensions, p.features, uab_types);
            let family_info = vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(vk_queue_family_index)
                .queue_priorities(&[1.0])
                .build();
            let family_infos = [family_info];

            let str_pointers = device_extensions
                .iter()
                .map(|&s| {
                    // Safe because `enabled_extensions` entries have static lifetime.
                    s.as_ptr()
                })
                .collect::<Vec<_>>();

            let pre_info = vk::DeviceCreateInfo::builder()
                .queue_create_infos(&family_infos)
                .enabled_extension_names(&str_pointers);
            let info = enabled_phd_features.add_to_device_create_builder(pre_info).build();
            let vk_device = ash_instance.create_device(vk_physical_device, &info, None).unwrap();
            let vk_queue = vk_device.get_device_queue(vk_queue_family_index, 0);

            (
                hal_adapter
                    .adapter
                    .device_from_raw(
                        vk_device.clone(),
                        true,
                        &device_extensions,
                        p.features,
                        uab_types,
                        vk_queue_family_index,
                        0,
                    )
                    .unwrap(),
                vk_queue,
                vk_device,
            )
        };

        let instance = wgpu::Instance::from_hal::<Vulkan>(hal_instance);
        let surface = instance.create_surface(&p.window);
        let adapter = instance.create_adapter_from_hal(hal_adapter);
        let (device, queue) = adapter
            .create_device_from_hal(
                hal_device,
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: p.features,
                    limits: p.limits.clone(),
                },
                None,
            )
            .unwrap();

        let cmd_pool = CmdPool::create(&ash_device, 0, 32);

        VulkanWGPU {
            instance,
            surface,
            adapter,
            device,
            queue,
            ash_device,
            vk_physical_device,
            ash_instance,
            vk_queue,
            vk_queue_family_index,
            cmd_pool,
        }
    }
}

fn add_if_doesnt_exist(v: &mut Vec<&'static CStr>, exts: impl IntoIterator<Item = &'static CStr>) {
    for ext in exts.into_iter() {
        let exists = v.iter().cloned().contains(&ext);
        if !exists {
            v.push(ext);
        }
    }
}

pub struct EyeData {
    pub texture: wgpu::Texture,
    pub depth_texture: wgpu::Texture,
    pub texture_view: wgpu::TextureView,
    pub depth_texture_view: wgpu::TextureView,
    pub raw_handle: vk::Image,
    pub width: u32,
    pub height: u32,
}

impl EyeData {
    pub fn create(device: &wgpu::Device, w: u32, h: u32) -> EyeData {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: w,
                height: h,
                ..Default::default()
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            label: None,
        });
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: w,
                height: h,
                ..Default::default()
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            label: None,
        });
        let depth_texture_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut raw_handle = vk::Image::null();
        unsafe {
            texture.as_hal::<Vulkan, _>(|v| {
                raw_handle = v.unwrap().raw_handle();
            });
        }

        EyeData {
            texture,
            depth_texture,
            texture_view,
            depth_texture_view,
            raw_handle,
            width: w,
            height: h,
        }
    }
}
