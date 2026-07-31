use eframe::{egui, App, Frame, NativeOptions};
use gilrs::{Button, Event, EventType, Gilrs};
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn main() {
    let mut options = NativeOptions::default();
    options.viewport = egui::ViewportBuilder::default()
        .with_title("PrismDeck Launcher")
        .with_fullscreen(true)
        .with_title_shown(false)
        .with_decorations(false);
    options.follow_system_theme = true;
    options.default_theme = eframe::Theme::Dark;
    let _ = eframe::run_native(
        "PrismDeck Launcher",
        options,
        Box::new(|_cc| Box::new(PrismLauncherApp::new())),
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ControllerStyle {
    Xbox,
    PlayStation,
    Generic,
}

impl ControllerStyle {
    fn confirm_button_label(&self) -> &'static str {
        match self {
            ControllerStyle::Xbox => "A",
            ControllerStyle::PlayStation => "Cross",
            ControllerStyle::Generic => "South",
        }
    }

    fn cancel_button_label(&self) -> &'static str {
        match self {
            ControllerStyle::Xbox => "B",
            ControllerStyle::PlayStation => "Circle",
            ControllerStyle::Generic => "East",
        }
    }

    fn refresh_button_label(&self) -> &'static str {
        match self {
            ControllerStyle::Xbox => "Y",
            ControllerStyle::PlayStation => "Triangle",
            ControllerStyle::Generic => "North",
        }
    }
}

#[derive(Clone)]
enum PrismInstanceKind {
    Native,
    Flatpak,
}

#[derive(Clone)]
struct PrismInstance {
    name: String,
    command: String,
    kind: PrismInstanceKind,
}

struct PrismLauncherApp {
    instances: Vec<PrismInstance>,
    selected_index: usize,
    message: String,
    controller_style: ControllerStyle,
    gilrs: Option<Gilrs>,
}

impl PrismLauncherApp {
    fn new() -> Self {
        let mut gilrs = Gilrs::new().ok();
        let controller_style = gilrs
            .as_mut()
            .and_then(Self::detect_style)
            .unwrap_or(ControllerStyle::Generic);

        let instances = Self::discover_instances();
        let selected_index = if instances.is_empty() { 0 } else { 0 };
        let message = if instances.is_empty() {
            "No Prism Launcher instances found. Use a controller or keyboard to refresh.".into()
        } else {
            "Use your controller to choose a Prism Launcher instance and press confirm.".into()
        };

        Self {
            instances,
            selected_index,
            message,
            controller_style,
            gilrs,
        }
    }

    fn detect_style(gilrs: &mut Gilrs) -> Option<ControllerStyle> {
        for (_id, gamepad) in gilrs.gamepads() {
            let name = gamepad.name().to_lowercase();
            if name.contains("xbox") || name.contains("x-box") || name.contains("series") {
                return Some(ControllerStyle::Xbox);
            }
            if name.contains("playstation")
                || name.contains("dualshock")
                || name.contains("dualsense")
                || name.contains("ps4")
                || name.contains("ps5")
            {
                return Some(ControllerStyle::PlayStation);
            }
        }
        None
    }

    fn discover_instances() -> Vec<PrismInstance> {
        let mut instances = Vec::new();
        if let Some(path) = Self::find_native_prism_launcher() {
            instances.push(PrismInstance {
                name: "Prism Launcher (native)".into(),
                command: path.to_string_lossy().into_owned(),
                kind: PrismInstanceKind::Native,
            });
        }
        if Self::flatpak_available() {
            instances.push(PrismInstance {
                name: "Prism Launcher (Flatpak)".into(),
                command: "flatpak run org.prismlauncher.PrismLauncher".into(),
                kind: PrismInstanceKind::Flatpak,
            });
        }
        instances
    }

    fn find_native_prism_launcher() -> Option<PathBuf> {
        let candidates = vec![
            "prism-launcher",
            "/usr/bin/prism-launcher",
            "/usr/local/bin/prism-launcher",
            "~/.local/bin/prism-launcher",
        ];

        for candidate in candidates {
            if candidate == "prism-launcher" {
                if let Ok(path) = which::which(candidate) {
                    return Some(path);
                }
            } else {
                let expanded = shellexpand::tilde(candidate).into_owned();
                let path = PathBuf::from(expanded);
                if path.exists() && path.is_file() {
                    return Some(path);
                }
            }
        }

        None
    }

    fn flatpak_available() -> bool {
        if which::which("flatpak").is_err() {
            return false;
        }
        Command::new("flatpak")
            .args(["info", "org.prismlauncher.PrismLauncher"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    fn launch_selected(&mut self) {
        if self.instances.is_empty() {
            self.message = "Nothing to launch. Refresh to detect Prism Launcher instances.".into();
            return;
        }

        let instance = &self.instances[self.selected_index];
        let result = if matches!(instance.kind, PrismInstanceKind::Flatpak) {
            Command::new("flatpak")
                .args(["run", "org.prismlauncher.PrismLauncher"])
                .spawn()
        } else {
            Command::new(&instance.command).spawn()
        };

        match result {
            Ok(_) => self.message = format!("Launched {}.", instance.name),
            Err(err) => self.message = format!("Failed to launch {}: {}", instance.name, err),
        }
    }

    fn refresh_instances(&mut self) {
        self.instances = Self::discover_instances();
        self.selected_index = self.instances.get(0).is_some().then_some(0).unwrap_or(0);
        self.message = if self.instances.is_empty() {
            "No Prism Launcher instances found. Confirm to refresh.".into()
        } else {
            "Instances refreshed. Use confirm to launch and cancel to quit.".into()
        };
    }

    fn handle_gamepad_events(&mut self) {
        if let Some(gilrs) = &mut self.gilrs {
            let mut events = Vec::new();
            while let Some(Event { event, .. }) = gilrs.next_event() {
                events.push(event);
            }

            for event in events {
                match event {
                    EventType::ButtonPressed(button, _) => match button {
                        Button::DPadDown => {
                            if !self.instances.is_empty() {
                                self.selected_index = (self.selected_index + 1) % self.instances.len();
                            }
                        }
                        Button::DPadUp => {
                            if !self.instances.is_empty() {
                                self.selected_index = if self.selected_index == 0 {
                                    self.instances.len() - 1
                                } else {
                                    self.selected_index - 1
                                };
                            }
                        }
                        Button::South => self.launch_selected(),
                        Button::East => std::process::exit(0),
                        Button::North => self.refresh_instances(),
                        _ => {}
                    },
                    EventType::Connected => {
                        self.controller_style = self
                            .gilrs
                            .as_mut()
                            .and_then(Self::detect_style)
                            .unwrap_or(ControllerStyle::Generic);
                    }
                    _ => {}
                }
            }
        }
    }
}

impl App for PrismLauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        self.handle_gamepad_events();
        self.controller_style = self
            .controller_style
            .clone();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("PrismDeck Controller Launcher");
            ui.label(&self.message);
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Navigate:");
                ui.label("D-Pad / Stick");
                ui.separator();
                ui.label("Select:");
                ui.label(self.controller_style.confirm_button_label());
                ui.separator();
                ui.label("Back:");
                ui.label(self.controller_style.cancel_button_label());
                ui.separator();
                ui.label("Refresh:");
                ui.label(self.controller_style.refresh_button_label());
            });
            ui.separator();

            if self.instances.is_empty() {
                ui.label("No Prism Launcher instances detected.");
            } else {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (index, instance) in self.instances.iter().enumerate() {
                        let selected = index == self.selected_index;
                        let frame = egui::Frame::canvas(ui.style())
                            .fill(if selected { ui.style().visuals.selection.bg_fill } else { ui.style().visuals.extreme_bg_color });
                        frame.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(if selected { "▶" } else { "" });
                                ui.vertical(|ui| {
                                    ui.label(&instance.name);
                                    ui.label(match instance.kind {
                                        PrismInstanceKind::Native => "Native executable",
                                        PrismInstanceKind::Flatpak => "Flatpak sandbox",
                                    });
                                });
                            });
                        });
                    }
                });
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.separator();
                ui.label("Made for Steam Big Picture. Press the back button or Esc to exit.");
            });
        });

        if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
            if !self.instances.is_empty() {
                self.selected_index = (self.selected_index + 1) % self.instances.len();
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
            if !self.instances.is_empty() {
                self.selected_index = if self.selected_index == 0 {
                    self.instances.len().saturating_sub(1)
                } else {
                    self.selected_index - 1
                };
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            self.launch_selected();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::R)) {
            self.refresh_instances();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            std::process::exit(0);
        }

        ctx.request_repaint();
    }
}
