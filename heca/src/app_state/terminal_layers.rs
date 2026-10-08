//! The GPU layers a terminal is drawn into, kept between frames.

pub struct RetainedTerminalLayer {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    physical_size: (u32, u32),
    pub render_key: u64,
    /// Signature of the inline-image placements last rendered into this layer.
    /// When it changes, only the affected image rows (old ∪ new) are repainted —
    /// images appear, move, clear, and animate without a full-pane repaint. See
    /// `terminal-09` / `terminal-task-23`.
    pub graphics_sig: u64,
    /// Visible row ranges the last-rendered inline images covered. Retained so a
    /// placement change or animation frame advance can damage the *old* rows too
    /// (otherwise a removed/moved image would leave stale pixels behind).
    pub image_rows: Vec<heca_core::backend::TerminalRowRange>,
}


impl RetainedTerminalLayer {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        render_key: u64,
    ) -> Self {
        let (texture, view) = make_terminal_texture(
            device,
            format,
            width,
            height,
            "terminal_layer",
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        Self {
            texture,
            view,
            physical_size: (width.max(1), height.max(1)),
            render_key,
            graphics_sig: 0,
            image_rows: Vec::new(),
        }
    }

    pub fn ensure_size(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> bool {
        let next_size = (width.max(1), height.max(1));
        if self.physical_size == next_size {
            return false;
        }
        let (texture, view) = make_terminal_texture(
            device,
            format,
            next_size.0,
            next_size.1,
            "terminal_layer",
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        self.texture = texture;
        self.view = view;
        self.physical_size = next_size;
        true
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }
}


pub struct RetainedTerminalScratch {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    physical_size: (u32, u32),
}


impl RetainedTerminalScratch {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let (texture, view) = make_terminal_texture(
            device,
            format,
            width,
            height,
            "terminal_layer_scratch",
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        Self {
            texture,
            view,
            physical_size: (width.max(1), height.max(1)),
        }
    }

    pub fn ensure_size(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) {
        let next_size = (width.max(1), height.max(1));
        if self.physical_size == next_size {
            return;
        }
        let (texture, view) = make_terminal_texture(
            device,
            format,
            next_size.0,
            next_size.1,
            "terminal_layer_scratch",
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        self.texture = texture;
        self.view = view;
        self.physical_size = next_size;
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }
}


fn make_terminal_texture(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    label: &str,
    usage: wgpu::TextureUsages,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

