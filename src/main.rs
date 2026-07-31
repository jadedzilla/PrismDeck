use eframe::{egui, App, Frame, NativeOptions};
use gilrs::{Button, Event, EventType, Gilrs};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
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
    instance_path: PathBuf,
    kind: PrismInstanceKind,
    icon_path: Option<PathBuf>,
}

#[derive(Clone)]
struct PrismModpack {
    name: String,
    path: PathBuf,
    icon_path: Option<PathBuf>,
}

struct PrismLauncherApp {
    native_launcher: Option<PathBuf>,
    instances: Vec<PrismInstance>,
    instance_textures: Vec<Option<egui::TextureHandle>>,
    selected_index: usize,
    message: String,
    controller_style: ControllerStyle,
    gilrs: Option<Gilrs>,
    window_focused: bool,
}

impl PrismLauncherApp {
    fn new() -> Self {
        let mut gilrs = Gilrs::new().ok();
        let controller_style = gilrs
            .as_mut()
            .and_then(Self::detect_style)
            .unwrap_or(ControllerStyle::Generic);

        let (native_launcher, instances) = Self::discover_instances();
        let selected_index = if instances.is_empty() { 0 } else { 0 };
        let message = if instances.is_empty() {
            "No Prism Launcher modpacks found. Use a controller or keyboard to refresh.".into()
        } else {
            "Use your controller to choose a Prism Launcher modpack and press confirm.".into()
        };

        Self {
            native_launcher,
            instances,
            instance_textures: Vec::new(),
            selected_index,
            message,
            controller_style,
            gilrs,
            window_focused: true,
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

    fn discover_instances() -> (Option<PathBuf>, Vec<PrismInstance>) {
        let native_launcher = Self::find_native_prism_launcher();
        let mut instances = Vec::new();

        for modpack in Self::discover_modpacks(&Self::native_instance_paths()) {
            instances.push(PrismInstance {
                name: modpack.name,
                instance_path: modpack.path,
                kind: PrismInstanceKind::Native,
                icon_path: modpack.icon_path,
            });
        }

        if Self::flatpak_available() {
            for modpack in Self::discover_modpacks(&Self::flatpak_instance_paths()) {
                instances.push(PrismInstance {
                    name: modpack.name,
                    instance_path: modpack.path,
                    kind: PrismInstanceKind::Flatpak,
                    icon_path: modpack.icon_path,
                });
            }
        }

        if instances.is_empty() {
            for modpack in Self::discover_modpacks_from_cfg() {
                instances.push(PrismInstance {
                    name: modpack.name,
                    instance_path: modpack.path,
                    kind: PrismInstanceKind::Native,
                    icon_path: modpack.icon_path,
                });
            }
        }

        (native_launcher, instances)
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

    fn native_instance_paths() -> Vec<PathBuf> {
        let mut paths = vec![
            "~/.local/share/PrismLauncher/instances",
            "~/.local/share/prismlauncher/instances",
            "~/.local/share/prism-launcher/instances",
            "~/.config/prism-launcher/instances",
            "~/.config/prismlauncher/instances",
            "~/.config/PrismLauncher/instances",
        ]
        .into_iter()
        .map(|path| PathBuf::from(shellexpand::tilde(path).into_owned()))
        .collect::<Vec<_>>();

        paths.extend(Self::discover_prism_roots("~/.local/share", 3, false));
        paths.extend(Self::discover_prism_roots("~/.config", 3, false));
        Self::unique_directories(paths)
    }

    fn flatpak_instance_paths() -> Vec<PathBuf> {
        let mut paths = vec![
            "~/.var/app/org.prismlauncher.PrismLauncher/.local/share/prismlauncher/instances",
            "~/.var/app/org.prismlauncher.PrismLauncher/.local/share/PrismLauncher/instances",
            "~/.var/app/org.prismlauncher.PrismLauncher/.local/share/prism-launcher/instances",
            "~/.var/app/org.prismlauncher.PrismLauncher/.config/prismlauncher/instances",
            "~/.var/app/org.prismlauncher.PrismLauncher/.config/prism-launcher/instances",
            "~/.var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher/instances",
            "~/.var/app/org.prismlauncher.PrismLauncher/data/prismlauncher/instances",
            "~/.var/app/org.prismlauncher.PrismLauncher/data/prism-launcher/instances",
        ]
        .into_iter()
        .map(|path| PathBuf::from(shellexpand::tilde(path).into_owned()))
        .collect::<Vec<_>>();

        if let Some(home) = env::var_os("HOME") {
            let app_root = PathBuf::from(&home).join(".var/app");
            paths.extend(Self::discover_prism_roots(
                app_root.to_string_lossy().as_ref(),
                4,
                true,
            ));
        }

        Self::unique_directories(paths)
    }

    fn discover_prism_roots(
        root: &str,
        max_depth: usize,
        include_flatpak_base: bool,
    ) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let root_path = PathBuf::from(shellexpand::tilde(root).into_owned());
        if !root_path.is_dir() {
            return paths;
        }

        let mut stack = vec![(root_path.clone(), 0)];
        while let Some((current, depth)) = stack.pop() {
            if depth > max_depth {
                continue;
            }

            if let Some(name) = current.file_name().and_then(|n| n.to_str()) {
                let name_lower = name.to_lowercase();
                if name_lower.contains("prismlauncher")
                    || name_lower.contains("prism-launcher")
                    || name_lower.contains("prismlauncher")
                {
                    let candidate = current.join("instances");
                    if candidate.is_dir() {
                        paths.push(candidate);
                    }
                    if include_flatpak_base {
                        let candidate = current.join(".local/share/prismlauncher/instances");
                        if candidate.is_dir() {
                            paths.push(candidate);
                        }
                        let candidate = current.join(".local/share/PrismLauncher/instances");
                        if candidate.is_dir() {
                            paths.push(candidate);
                        }
                        let candidate = current.join(".local/share/prism-launcher/instances");
                        if candidate.is_dir() {
                            paths.push(candidate);
                        }
                        let candidate = current.join(".config/prismlauncher/instances");
                        if candidate.is_dir() {
                            paths.push(candidate);
                        }
                        let candidate = current.join(".config/prism-launcher/instances");
                        if candidate.is_dir() {
                            paths.push(candidate);
                        }
                        let candidate = current.join(".config/PrismLauncher/instances");
                        if candidate.is_dir() {
                            paths.push(candidate);
                        }
                        let candidate = current.join("data/PrismLauncher/instances");
                        if candidate.is_dir() {
                            paths.push(candidate);
                        }
                        let candidate = current.join("data/prismlauncher/instances");
                        if candidate.is_dir() {
                            paths.push(candidate);
                        }
                        let candidate = current.join("data/prism-launcher/instances");
                        if candidate.is_dir() {
                            paths.push(candidate);
                        }
                    }
                }
            }

            if depth < max_depth {
                if let Ok(entries) = fs::read_dir(&current) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            stack.push((path, depth + 1));
                        }
                    }
                }
            }
        }

        paths
    }

    fn unique_directories(paths: Vec<PathBuf>) -> Vec<PathBuf> {
        let mut seen = HashSet::new();
        let mut unique = Vec::new();

        for path in paths {
            let normalized = if let Ok(canonical) = fs::canonicalize(&path) {
                canonical
            } else {
                path.clone()
            };
            if seen.insert(normalized) {
                unique.push(path);
            }
        }

        unique
    }

    fn discover_modpacks(paths: &[PathBuf]) -> Vec<PrismModpack> {
        let mut instances = Vec::new();
        let paths = Self::unique_directories(paths.to_vec());

        for path in paths {
            if !path.exists() || !path.is_dir() {
                continue;
            }
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    if !entry_path.is_dir() {
                        continue;
                    }
                    let name = Self::read_modpack_name(&entry_path).unwrap_or_else(|| {
                        entry_path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned()
                    });
                    let icon_path = Self::read_modpack_icon_path(&entry_path);
                    instances.push(PrismModpack {
                        name,
                        path: entry_path,
                        icon_path,
                    });
                }
            }
        }

        instances
    }

    fn discover_modpacks_from_cfg() -> Vec<PrismModpack> {
        let mut instances = Vec::new();
        let mut candidate_dirs = Vec::new();

        if let Some(home) = env::var_os("HOME") {
            let roots = [
                PathBuf::from(&home).join(".local/share"),
                PathBuf::from(&home).join(".config"),
                PathBuf::from(&home).join(".var/app"),
            ];

            for root in roots {
                candidate_dirs.extend(Self::discover_directories_with_instance_cfg(&root, 5));
            }
        }

        candidate_dirs = Self::unique_directories(candidate_dirs);

        for instance_dir in candidate_dirs {
            let name = Self::read_modpack_name(&instance_dir).unwrap_or_else(|| {
                instance_dir
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            });
            let icon_path = Self::read_modpack_icon_path(&instance_dir);
            instances.push(PrismModpack {
                name,
                path: instance_dir,
                icon_path,
            });
        }

        instances
    }

    fn discover_directories_with_instance_cfg(root: &PathBuf, max_depth: usize) -> Vec<PathBuf> {
        let mut matches = Vec::new();
        if !root.is_dir() {
            return matches;
        }

        let mut stack = vec![(root.clone(), 0)];
        while let Some((current, depth)) = stack.pop() {
            if depth > max_depth {
                continue;
            }

            if let Ok(entries) = fs::read_dir(&current) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        stack.push((path, depth + 1));
                    } else if path.file_name().and_then(|n| n.to_str()) == Some("instance.cfg") {
                        if let Some(parent) = path.parent() {
                            matches.push(parent.to_path_buf());
                        }
                    }
                }
            }
        }

        matches
    }

    fn read_modpack_name(instance_dir: &PathBuf) -> Option<String> {
        let cfg_paths = [
            instance_dir.join("instance.cfg"),
            instance_dir.join(".minecraft/instance.cfg"),
        ];
        for path in cfg_paths {
            if let Ok(contents) = fs::read_to_string(&path) {
                for line in contents.lines() {
                    if let Some(name) = line.strip_prefix("name=") {
                        return Some(name.trim().to_owned());
                    }
                    if let Some(name) = line.strip_prefix("displayName=") {
                        return Some(name.trim().to_owned());
                    }
                }
            }
        }
        None
    }

    fn read_modpack_icon_path(instance_dir: &PathBuf) -> Option<PathBuf> {
        let icon_candidates = [
            "icon.png",
            "icon.jpg",
            "icon.jpeg",
            "icon.bmp",
            "logo.png",
            "logo.jpg",
            "instance.png",
            "instance_icon.png",
            "minecraft/icon.png",
            "minecraft/icon.jpg",
            "minecraft/icon.jpeg",
            "minecraft/logo.png",
        ];

        for candidate in icon_candidates {
            let candidate_path = instance_dir.join(candidate);
            if candidate_path.is_file() {
                return Some(candidate_path);
            }
        }

        let cfg_paths = [
            instance_dir.join("instance.cfg"),
            instance_dir.join(".minecraft/instance.cfg"),
        ];
        for path in cfg_paths {
            if let Ok(contents) = fs::read_to_string(&path) {
                for line in contents.lines() {
                    if let Some(value) = line.strip_prefix("icon=") {
                        let icon_path = value.trim();
                        if !icon_path.is_empty() {
                            let icon_path = PathBuf::from(icon_path);
                            let icon_path = if icon_path.is_absolute() {
                                icon_path
                            } else {
                                instance_dir.join(icon_path)
                            };
                            if icon_path.is_file() {
                                return Some(icon_path);
                            }
                        }
                    }
                }
            }
        }

        let minecraft_icon = instance_dir.join(".minecraft/icon.png");
        if minecraft_icon.is_file() {
            return Some(minecraft_icon);
        }

        None
    }

    fn ensure_instance_textures(&mut self, ctx: &egui::Context) {
        if self.instance_textures.len() != self.instances.len() {
            self.instance_textures = vec![None; self.instances.len()];
        }

        for (index, instance) in self.instances.iter().enumerate() {
            if self.instance_textures[index].is_none() {
                self.instance_textures[index] = instance
                    .icon_path
                    .as_ref()
                    .and_then(|path| Self::load_icon_texture(ctx, path));
            }
        }
    }

    fn load_icon_texture(ctx: &egui::Context, icon_path: &PathBuf) -> Option<egui::TextureHandle> {
        let image = image::open(icon_path).ok()?.to_rgba8();
        let size = [image.width() as usize, image.height() as usize];
        let pixels = image.into_raw();
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
        Some(ctx.load_texture(icon_path.to_string_lossy(), color_image, Default::default()))
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
            self.message = "Nothing to launch. Refresh to detect Prism Launcher modpacks.".into();
            return;
        }

        let instance = &self.instances[self.selected_index];
        let instance_id = instance
            .instance_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let instance_root = instance
            .instance_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let launcher_dir = instance_root
            .parent()
            .unwrap_or_else(|| instance_root.as_path());

        let result = match instance.kind {
            PrismInstanceKind::Flatpak => Command::new("flatpak")
                .args([
                    "run",
                    "org.prismlauncher.PrismLauncher",
                    "--launch",
                    instance_id,
                    "--dir",
                    &launcher_dir.to_string_lossy(),
                ])
                .spawn(),
            PrismInstanceKind::Native => {
                let launcher = self
                    .native_launcher
                    .as_deref()
                    .map(|path| path.to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("prism-launcher"));
                Command::new(launcher)
                    .args([
                        "--launch",
                        instance_id,
                        "--dir",
                        &launcher_dir.to_string_lossy(),
                    ])
                    .spawn()
            }
        };

        match result {
            Ok(_) => self.message = format!("Launched {}.", instance.name),
            Err(err) => self.message = format!("Failed to launch {}: {}", instance.name, err),
        }
    }

    fn refresh_instances(&mut self) {
        let (_native_launcher, instances) = Self::discover_instances();
        self.native_launcher = _native_launcher;
        self.instances = instances;
        self.instance_textures.clear();
        self.selected_index = self.instances.get(0).is_some().then_some(0).unwrap_or(0);
        self.message = if self.instances.is_empty() {
            "No Prism Launcher modpacks found. Confirm to refresh.".into()
        } else {
            "Modpack instances refreshed. Use confirm to launch and cancel to quit.".into()
        };
    }

    fn update_window_focus(&mut self, ctx: &egui::Context) {
        ctx.input(|input| {
            for event in input.raw.events.iter() {
                if let egui::Event::WindowFocused(focused) = event {
                    self.window_focused = *focused;
                }
            }
        });
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
                                self.selected_index =
                                    (self.selected_index + 1) % self.instances.len();
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
        self.update_window_focus(ctx);
        if self.window_focused {
            self.handle_gamepad_events();
        }
        self.ensure_instance_textures(ctx);
        self.controller_style = self.controller_style.clone();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("PrismDeck Controller Launcher");
            ui.label("Detected Prism Launcher modpacks from native and Flatpak installations.");
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
                ui.label("No Prism Launcher modpacks detected.");
            } else {
                ui.label("Select a modpack from the shelf below.");
                ui.add_space(10.0);
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (index, instance) in self.instances.iter().enumerate() {
                            let selected = index == self.selected_index;
                            let card_size = egui::vec2(260.0, 260.0);
                            ui.add_space(8.0);
                            ui.vertical(|ui| {
                                if selected {
                                    ui.label(
                                        egui::RichText::new(&instance.name).strong().size(18.0),
                                    );
                                } else {
                                    ui.add_space(24.0);
                                }
                                let (rect, _response) =
                                    ui.allocate_exact_size(card_size, egui::Sense::hover());
                                let fill_color = if selected {
                                    ui.style().visuals.selection.bg_fill
                                } else {
                                    ui.style().visuals.extreme_bg_color
                                };
                                let rounding = egui::Rounding::same(24.0);
                                let stroke = if selected {
                                    egui::Stroke::new(3.0_f32, ui.visuals().selection.stroke.color)
                                } else {
                                    egui::Stroke::new(
                                        1.0_f32,
                                        ui.visuals().widgets.noninteractive.bg_stroke.color,
                                    )
                                };

                                if let Some(Some(texture)) = self.instance_textures.get(index) {
                                    let mut shape = egui::epaint::RectShape::new(
                                        rect,
                                        rounding,
                                        egui::Color32::WHITE,
                                        stroke,
                                    );
                                    shape.fill_texture_id = texture.id();
                                    shape.uv = egui::Rect::from_min_max(
                                        egui::Pos2::new(0.0, 0.0),
                                        egui::Pos2::new(1.0, 1.0),
                                    );
                                    ui.painter().add(egui::Shape::Rect(shape));
                                } else {
                                    ui.painter().rect_filled(rect, rounding, fill_color);
                                    ui.painter().rect_stroke(rect, rounding, stroke);
                                }

                                let overlay_color = if self
                                    .instance_textures
                                    .get(index)
                                    .and_then(|t| t.as_ref())
                                    .is_some()
                                {
                                    egui::Color32::from_rgba_premultiplied(0, 0, 0, 80)
                                } else {
                                    egui::Color32::TRANSPARENT
                                };
                                if overlay_color != egui::Color32::TRANSPARENT {
                                    ui.painter().rect_filled(rect, rounding, overlay_color);
                                }

                                let text_color = if selected {
                                    ui.visuals().strong_text_color()
                                } else {
                                    ui.visuals().text_color()
                                };

                                ui.painter().text(
                                    rect.center() + egui::vec2(0.0, -10.0),
                                    egui::Align2::CENTER_CENTER,
                                    match instance.kind {
                                        PrismInstanceKind::Native => "Native",
                                        PrismInstanceKind::Flatpak => "Flatpak",
                                    },
                                    egui::TextStyle::Body.resolve(ui.style()),
                                    text_color,
                                );

                                let hint_text = if selected {
                                    "Selected"
                                } else {
                                    "Use D-pad to move and confirm to launch."
                                };
                                ui.painter().text(
                                    rect.center_bottom() + egui::vec2(0.0, -22.0),
                                    egui::Align2::CENTER_BOTTOM,
                                    hint_text,
                                    egui::TextStyle::Body.resolve(ui.style()),
                                    text_color,
                                );
                            });
                        }
                        ui.add_space(8.0);
                    });
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
