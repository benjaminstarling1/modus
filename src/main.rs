mod app;
mod create_nodes;
mod csys_builder;
mod export_video;
mod data;
mod fft;
mod persist;
mod table;
mod time_plot;
mod viewport;

use app::App;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("modus")
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([800.0, 600.0]),
        depth_buffer: 24,
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(
                eframe::egui_wgpu::WgpuSetupCreateNew {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    ..Default::default()
                },
            ),
            ..Default::default()
        },
        ..Default::default()
    };

    eframe::run_native(
        "modus",
        native_options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
