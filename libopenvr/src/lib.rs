use ash::vk::Handle;
use glam::{Mat4, Vec4};
use lazy_static::lazy_static;
use std::{
    ffi::{c_void, CStr, CString},
    mem::MaybeUninit,
    ptr,
    sync::Mutex,
};

#[derive(Copy, Clone)]
pub enum ApplicationType {
    Scene = sys::EVRApplicationType_VRApplication_Scene as isize,
}

#[derive(Copy, Clone, Debug)]
pub enum Eye {
    Left = sys::EVREye_Eye_Left as isize,
    Right = sys::EVREye_Eye_Right as isize,
}

#[derive(Copy, Clone)]
pub enum TrackedDeviceClass {
    Invalid = sys::ETrackedDeviceClass_TrackedDeviceClass_Invalid as isize,
    HMD = sys::ETrackedDeviceClass_TrackedDeviceClass_HMD as isize,
    Controller = sys::ETrackedDeviceClass_TrackedDeviceClass_Controller as isize,
    GenericTracker = sys::ETrackedDeviceClass_TrackedDeviceClass_GenericTracker as isize,
    TrackingReference = sys::ETrackedDeviceClass_TrackedDeviceClass_TrackingReference as isize,
    DisplayRedirect = sys::ETrackedDeviceClass_TrackedDeviceClass_DisplayRedirect as isize,
}

fn load<T>(suffix: &[u8]) -> *const T {
    let mut magic = Vec::from(b"FnTable:".as_ref());
    magic.extend(suffix);
    let mut error = sys::EVRInitError_VRInitError_None;
    let result = unsafe { sys::VR_GetGenericInterface(magic.as_ptr() as *const i8, &mut error) };
    if error != sys::EVRInitError_VRInitError_None {
        let msg = unsafe { CStr::from_ptr(sys::VR_GetVRInitErrorAsEnglishDescription(error)) }
            .to_str()
            .unwrap();
        panic!("openvr subsystem init failure: {}", msg);
    }
    result as *const T
}

pub struct System(&'static sys::VR_IVRSystem_FnTable);
pub struct Compositor(&'static sys::VR_IVRCompositor_FnTable);

pub struct Context {
    pub system: System,
    pub compositor: Compositor,
}

fn hmd_matrix44_to_glam(m: sys::HmdMatrix44_t) -> Mat4 {
    Mat4::from_cols_array_2d(&m.m).transpose()
}

fn hmd_matrix34_to_glam(m: sys::HmdMatrix34_t) -> Mat4 {
    let mut result = Mat4::IDENTITY;
    *result.col_mut(0) = Vec4::from(m.m[0]);
    *result.col_mut(1) = Vec4::from(m.m[1]);
    *result.col_mut(2) = Vec4::from(m.m[2]);
    result.transpose()
}

lazy_static! {
    static ref INSTANCE_EXTENSIONS: Mutex<Vec<CString>> = Mutex::new(Vec::new());
    static ref DEVICE_EXTENSIONS: Mutex<Vec<CString>> = Mutex::new(Vec::new());
}

impl Context {
    pub fn create(typ: ApplicationType) -> Box<Context> {
        unsafe {
            let mut error = sys::EVRInitError_VRInitError_None;
            sys::VR_InitInternal(&mut error, typ as sys::EVRApplicationType);
            if error != sys::EVRInitError_VRInitError_None {
                let msg = CStr::from_ptr(sys::VR_GetVRInitErrorAsEnglishDescription(error))
                    .to_str()
                    .unwrap();
                panic!("openvr init failure: {}", msg);
            }
            Box::new(Context {
                system: System(&*load(sys::IVRSystem_Version)),
                compositor: Compositor(&*load(sys::IVRCompositor_Version)),
            })
        }
    }

    pub fn shutdown(&self) {
        unsafe {
            sys::VR_ShutdownInternal();
        }
    }
}

impl System {
    pub fn get_projection_matrix(&self, eye: Eye, near: f32, far: f32) -> Mat4 {
        unsafe { hmd_matrix44_to_glam(self.0.GetProjectionMatrix.unwrap()(eye as sys::EVREye, near, -far)) }
    }

    pub fn get_eye_to_head_transform(&self, eye: Eye) -> Mat4 {
        unsafe { hmd_matrix34_to_glam(self.0.GetEyeToHeadTransform.unwrap()(eye as sys::EVREye)) }
    }

    pub fn recommended_render_target_size(&self) -> (u32, u32) {
        let mut result: (u32, u32) = (0, 0);
        unsafe {
            self.0.GetRecommendedRenderTargetSize.unwrap()(&mut result.0, &mut result.1);
        }
        result
    }

    pub fn get_output_device_for_vulkan(&self, instance: ash::vk::Instance) -> ash::vk::PhysicalDevice {
        let mut result: u64 = 0;
        unsafe {
            self.0.GetOutputDevice.unwrap()(
                &mut result,
                sys::ETextureType_TextureType_Vulkan,
                instance.as_raw() as *mut sys::VkInstance_T,
            )
        }
        ash::vk::PhysicalDevice::from_raw(result)
    }
}

pub struct VulkanTextureData {
    pub image: ash::vk::Image,
    pub device: ash::vk::Device,
    pub physical_device: ash::vk::PhysicalDevice,
    pub instance: ash::vk::Instance,
    pub queue: ash::vk::Queue,
    pub queue_family_index: u32,
    pub width: u32,
    pub height: u32,
    pub format: ash::vk::Format,
    pub sample_count: ash::vk::SampleCountFlags,
}

pub struct TextureBounds {
    pub u_min: f32,
    pub v_min: f32,
    pub u_max: f32,
    pub v_max: f32,
}

unsafe fn cstring_vec_to_cstr_vec(v: &Vec<CString>) -> Vec<&'static CStr> {
    v.iter().map(|v| std::mem::transmute(v.as_c_str())).collect()
}

impl Compositor {
    pub fn get_vulkan_instance_extensions_required(&self) -> Vec<&'static CStr> {
        let mut vec = INSTANCE_EXTENSIONS.lock().unwrap();
        unsafe {
            if vec.len() == 0 {
                let size = self.0.GetVulkanInstanceExtensionsRequired.unwrap()(ptr::null_mut(), 0);
                let mut buf: Vec<u8> = Vec::with_capacity(size as usize);
                buf.resize(size as usize, 0);
                self.0.GetVulkanInstanceExtensionsRequired.unwrap()(buf.as_mut_ptr() as *mut i8, size);
                buf.truncate((size - 1) as usize);
                buf.split(|&v| v as u8 == b' ').for_each(|v| {
                    vec.push(CString::new(v).unwrap());
                });
            }
            cstring_vec_to_cstr_vec(&vec)
        }
    }

    pub fn get_vulkan_device_extensions_required(&self, device: ash::vk::PhysicalDevice) -> Vec<&'static CStr> {
        let mut vec = DEVICE_EXTENSIONS.lock().unwrap();
        unsafe {
            if vec.len() == 0 {
                let size = self.0.GetVulkanDeviceExtensionsRequired.unwrap()(
                    device.as_raw() as *mut sys::VkPhysicalDevice_T,
                    ptr::null_mut(),
                    0,
                );
                let mut buf: Vec<u8> = Vec::with_capacity(size as usize);
                buf.resize(size as usize, 0);
                self.0.GetVulkanDeviceExtensionsRequired.unwrap()(
                    device.as_raw() as *mut sys::VkPhysicalDevice_T,
                    buf.as_mut_ptr() as *mut i8,
                    size,
                );
                buf.truncate((size - 1) as usize);
                buf.split(|&v| v as u8 == b' ').for_each(|v| {
                    vec.push(CString::new(v).unwrap());
                });
            }
            cstring_vec_to_cstr_vec(&vec)
        }
    }

    pub fn wait_get_hmd_pose(&self) -> Mat4 {
        unsafe {
            let mut poses: [sys::TrackedDevicePose_t; 1] = MaybeUninit::zeroed().assume_init();
            self.0.WaitGetPoses.unwrap()(poses.as_mut_ptr(), 1, ptr::null_mut(), 0);
            hmd_matrix34_to_glam(poses[0].mDeviceToAbsoluteTracking)
        }
    }

    pub fn submit_opengl(&self, eye: Eye, texture: i32) {
        unsafe {
            let mut texture = sys::Texture_t {
                handle: texture as usize as *mut c_void,
                eType: sys::ETextureType_TextureType_OpenGL,
                eColorSpace: sys::EColorSpace_ColorSpace_Gamma,
            };
            self.0.Submit.unwrap()(eye as sys::EVREye, &mut texture, ptr::null_mut(), 0);
        }
    }

    pub fn submit_vulkan(&self, eye: Eye, texture_data: &VulkanTextureData, texture_bounds: &TextureBounds) {
        unsafe {
            let mut tex_data = sys::VRVulkanTextureData_t {
                m_nImage: texture_data.image.as_raw(),
                m_pDevice: texture_data.device.as_raw() as *mut sys::VkDevice_T,
                m_pPhysicalDevice: texture_data.physical_device.as_raw() as *mut sys::VkPhysicalDevice_T,
                m_pInstance: texture_data.instance.as_raw() as *mut sys::VkInstance_T,
                m_pQueue: texture_data.queue.as_raw() as *mut sys::VkQueue_T,
                m_nQueueFamilyIndex: texture_data.queue_family_index,
                m_nWidth: texture_data.width,
                m_nHeight: texture_data.height,
                m_nFormat: texture_data.format.as_raw() as u32,
                m_nSampleCount: texture_data.sample_count.as_raw(),
            };
            let mut tex_bounds = sys::VRTextureBounds_t {
                uMax: texture_bounds.u_max,
                uMin: texture_bounds.u_min,
                vMax: texture_bounds.v_max,
                vMin: texture_bounds.v_min,
            };
            let mut texture = sys::Texture_t {
                handle: &mut tex_data as *mut sys::VRVulkanTextureData_t as *mut c_void,
                eType: sys::ETextureType_TextureType_Vulkan,
                eColorSpace: sys::EColorSpace_ColorSpace_Auto,
            };
            self.0.Submit.unwrap()(eye as sys::EVREye, &mut texture, &mut tex_bounds, 0);
        }
    }
}
