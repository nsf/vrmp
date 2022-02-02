use ash::vk;

// basic vulkan command pool with a
struct CmdBufAwaitingList {
    fence: vk::Fence,
    cmd_bufs: Vec<vk::CommandBuffer>,
}

pub struct CmdPool {
    cmd_pool: vk::CommandPool,
    free_cmd_bufs: Vec<vk::CommandBuffer>,

    active_awaiting_lists: Vec<CmdBufAwaitingList>,
    free_awaiting_lists: Vec<CmdBufAwaitingList>,
    current_awaiting_list: CmdBufAwaitingList,
}

impl CmdPool {
    pub unsafe fn create(device: &ash::Device, queue_family_index: u32, n: u32) -> CmdPool {
        let vk_info = vk::CommandPoolCreateInfo::builder()
            .queue_family_index(queue_family_index)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .build();
        let cmd_pool = device.create_command_pool(&vk_info, None).unwrap();

        let vk_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(cmd_pool)
            .command_buffer_count(n)
            .build();

        let free_cmd_bufs = device.allocate_command_buffers(&vk_info).unwrap();

        let vk_info = vk::FenceCreateInfo::builder().build();
        let current_fence = device.create_fence(&vk_info, None).unwrap();

        CmdPool {
            cmd_pool,
            free_cmd_bufs,

            active_awaiting_lists: Vec::new(),
            free_awaiting_lists: Vec::new(),
            current_awaiting_list: CmdBufAwaitingList {
                fence: current_fence,
                cmd_bufs: Vec::new(),
            },
        }
    }

    pub unsafe fn shutdown(&mut self, device: &ash::Device) {
        log::info!("waiting for {} awaiting lists", self.active_awaiting_lists.len());
        let fences = self
            .active_awaiting_lists
            .drain(..)
            .map(|list| list.fence)
            .collect::<Vec<_>>();
        let ten_seconds = std::time::Duration::from_secs(10).as_nanos();
        let result = device.wait_for_fences(&fences, true, ten_seconds as u64);
        if result.is_err() {
            log::error!("timeout waiting for awaiting list");
        }
        for f in fences {
            device.destroy_fence(f, None);
        }
        for l in &self.free_awaiting_lists {
            device.destroy_fence(l.fence, None);
        }
        device.destroy_fence(self.current_awaiting_list.fence, None);
        device.destroy_command_pool(self.cmd_pool, None);
    }

    unsafe fn allocate_current_awaiting_list(&mut self, device: &ash::Device) -> CmdBufAwaitingList {
        match self.free_awaiting_lists.pop() {
            Some(list) => list,
            None => {
                let num_active_lists = self.active_awaiting_lists.len();
                log::info!("allocating new awaiting list (active lists: {})", num_active_lists);
                let vk_info = vk::FenceCreateInfo::builder().build();
                CmdBufAwaitingList {
                    fence: device.create_fence(&vk_info, None).unwrap(),
                    cmd_bufs: Vec::new(),
                }
            }
        }
    }

    unsafe fn evaluate_active_fences(&mut self, device: &ash::Device) {
        let mut i = 0;
        while i < self.active_awaiting_lists.len() {
            let f = &self.active_awaiting_lists[i];
            let is_signaled = device.get_fence_status(f.fence).unwrap();

            if is_signaled {
                let mut f = self.active_awaiting_lists.remove(i);
                device.reset_fences(&[f.fence]).unwrap();
                self.free_cmd_bufs.extend(f.cmd_bufs.drain(..));
                self.free_awaiting_lists.push(f);
            } else {
                i += 1;
            }
        }
    }

    pub unsafe fn submit_frame(&mut self, device: &ash::Device, queue: vk::Queue) {
        assert!(self.current_awaiting_list.fence != vk::Fence::null());
        device
            .queue_submit(queue, &[], self.current_awaiting_list.fence)
            .unwrap();

        let new_list = self.allocate_current_awaiting_list(device);
        let list = std::mem::replace(&mut self.current_awaiting_list, new_list);
        self.active_awaiting_lists.push(list);

        self.evaluate_active_fences(device);
    }

    pub unsafe fn get_buf(&mut self) -> vk::CommandBuffer {
        let buf = self
            .free_cmd_bufs
            .pop()
            .expect("cmd pool is out of free buffers, make sure you return them or preallocate more");
        self.current_awaiting_list.cmd_bufs.push(buf);
        buf
    }
}
