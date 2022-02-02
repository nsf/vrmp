## VR media player for linux

Very early development access version.

I'll drop more notes here once it's ready for more publicity.

Some details:

 - Uses wgpu (assumes vulkan wgpu-hal).
 - Uses libmpv.
 - Uses openvr (aka SteamVR).
 - Written in rust.
 - Uses opengl <-> vulkan interop to bridge libmpv (egl/opengl) and wgpu (vulkan/wgpu).
 - Vulkan wgpu-hal hacks are used to talk to openvr.
 - Uses imgui for gui.
 - Mouse and keyboard only for now.
 - Fully functional without VR device (in fact --vr option enables VR, runs w/o VR by default).
 - When w/o VR use WASD and mouse to navigate.
 - Right click to show the UI.
 - Spacebar to reset origin.

 Screenshot:

 ![vrmp](https://raw.github.com/nsf/vrmp/master/screenshot/vrmp.jpg)