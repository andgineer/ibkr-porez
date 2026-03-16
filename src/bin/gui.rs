use eframe::egui;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn log_file_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("ibkr-porez")
        .join("gui.log")
}

fn setup_panic_hook() {
    let log_path = log_file_path();
    std::panic::set_hook(Box::new(move |info| {
        let msg = format!(
            "{}\n{info}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        );
        let _ = std::fs::create_dir_all(log_path.parent().unwrap());
        let _ = std::fs::write(&log_path, &msg);
        eprintln!("{msg}");
    }));
}

fn log_error(msg: &str) {
    eprintln!("{msg}");
    let log_path = log_file_path();
    let _ = std::fs::create_dir_all(log_path.parent().unwrap());
    let timestamped = format!(
        "{}\n{msg}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );
    let _ = std::fs::write(&log_path, &timestamped);
}

fn make_viewport() -> egui::ViewportBuilder {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1100.0, 700.0])
        .with_min_inner_size([800.0, 500.0]);

    if let Some(icon) = ibkr_porez::gui::icon::load_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }
    viewport
}

fn wgpu_adapter_selector() -> eframe::egui_wgpu::NativeAdapterSelectorMethod {
    use eframe::wgpu;
    std::sync::Arc::new(
        |adapters: &[wgpu::Adapter], surface: Option<&wgpu::Surface<'_>>| {
            let compatible: Vec<&wgpu::Adapter> = if let Some(surface) = surface {
                adapters
                    .iter()
                    .filter(|a| a.is_surface_supported(surface))
                    .collect()
            } else {
                adapters.iter().collect()
            };

            if compatible.is_empty() {
                return Err("no compatible adapters found".into());
            }

            // Prefer hardware, accept software (WARP) as fallback
            let pick = compatible
                .iter()
                .find(|a| !matches!(a.get_info().device_type, wgpu::DeviceType::Cpu))
                .or(compatible.first())
                .unwrap();

            Ok((*pick).clone())
        },
    )
}

fn main() {
    if std::env::var("IBKR_POREZ_DRY_RUN").is_ok() {
        eprintln!("GUI launched (dry run)");
        return;
    }

    setup_panic_hook();

    let title = format!("IBKR Porez v{VERSION}");

    let wgpu_options = eframe::egui_wgpu::WgpuConfiguration {
        wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(
            eframe::egui_wgpu::WgpuSetupCreateNew {
                native_adapter_selector: Some(wgpu_adapter_selector()),
                ..Default::default()
            },
        ),
        ..Default::default()
    };

    let options = eframe::NativeOptions {
        viewport: make_viewport().with_title(&title),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options,
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        &title,
        options,
        Box::new(|_cc| Ok(Box::new(ibkr_porez::gui::app::App::new()))),
    ) {
        log_error(&format!("GUI failed to start: {e}"));
        std::process::exit(1);
    }
}
