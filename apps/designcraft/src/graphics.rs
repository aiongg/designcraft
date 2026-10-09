//! Windows instance configuration, applied before wgpu can initialize any driver.

use eframe::{NativeOptions, egui_wgpu::WgpuSetup, wgpu::Backends};

pub fn configure(options: &mut NativeOptions, backend_override: Option<Backends>) {
    if let WgpuSetup::CreateNew(create) = &mut options.wgpu_options.wgpu_setup {
        // eframe's default includes GL. Creating that backend can crash inside
        // AMD's atio6axx.dll before adapter selection or Rust error handling.
        // Match the verified WGPU_BACKEND=dx12 workaround without mutating
        // process environment variables or initializing GL as a fallback.
        create.instance_descriptor.backends = backend_override.unwrap_or(Backends::DX12);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn configured_backends(backend_override: Option<Backends>) -> Backends {
        let mut options = NativeOptions::default();
        configure(&mut options, backend_override);
        let WgpuSetup::CreateNew(create) = options.wgpu_options.wgpu_setup else {
            panic!("default native options must create a wgpu instance");
        };
        create.instance_descriptor.backends
    }

    #[test]
    fn windows_default_initializes_only_dx12() {
        // Adapter preference is too late: enabling GL can crash inside atio6axx.dll
        // while creating the instance, even when a different adapter is selected.
        assert_eq!(configured_backends(None), Backends::DX12);
    }

    #[test]
    fn explicit_backend_override_is_preserved() {
        for backends in [Backends::VULKAN, Backends::GL, Backends::DX12, Backends::VULKAN | Backends::DX12, Backends::empty()] {
            assert_eq!(configured_backends(Some(backends)), backends);
        }
    }
}
