mod tile;
mod style;
mod settings_panel;

use fluid_core::sensor_data::SensorSnapshot;
use fluid_core::settings::{AppSettings, Orientation, TempUnit};
use fluid_sensor::SensorPoller;
use iced::widget::{column, container, mouse_area, row};
use iced::{window, Border, Element, Length, Size, Subscription, Task, Theme};
use std::collections::BTreeMap;
use std::time::Duration;
use style::Palette;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    TrayIcon, TrayIconBuilder,
};

const VERTICAL_SIZE: Size = Size::new(240.0, 330.0);
const HORIZONTAL_SIZE: Size = Size::new(820.0, 86.0);
const SETTINGS_SIZE: Size = Size::new(280.0, 430.0);

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    iced::daemon("fluidMonitor", App::update, App::view)
        .subscription(App::subscription)
        .theme(App::theme)
        .run_with(App::new)
}

fn make_tray_icon() -> tray_icon::Icon {
    const SIZE: u32 = 32;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let corner = 6i32;
            let (xi, yi, s) = (x as i32, y as i32, SIZE as i32);
            let in_corner = (xi < corner && yi < corner && (corner - xi).pow(2) + (corner - yi).pow(2) > corner.pow(2))
                || (xi >= s - corner && yi < corner && (xi - (s - corner)).pow(2) + (corner - yi).pow(2) > corner.pow(2))
                || (xi < corner && yi >= s - corner && (corner - xi).pow(2) + (yi - (s - corner)).pow(2) > corner.pow(2))
                || (xi >= s - corner && yi >= s - corner && (xi - (s - corner)).pow(2) + (yi - (s - corner)).pow(2) > corner.pow(2));
            if in_corner {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                rgba.extend_from_slice(&[77, 153, 255, 255]);
            }
        }
    }
    tray_icon::Icon::from_rgba(rgba, SIZE, SIZE).expect("icon from rgba")
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum WindowKind {
    Widget,
    Settings,
}

struct App {
    settings: AppSettings,
    snapshot: SensorSnapshot,
    poller: Option<SensorPoller>,
    windows: BTreeMap<window::Id, WindowKind>,
    _tray: TrayIcon,
    settings_id: tray_icon::menu::MenuId,
    exit_id: tray_icon::menu::MenuId,
}

#[derive(Debug, Clone)]
enum Message {
    SensorTick,
    TrayPoll,
    DragWindow(window::Id),
    WindowOpened(window::Id, WindowKind),
    WindowClosed(window::Id),
    WindowMoved(window::Id, iced::Point),
    SaveClose,
    ResetDefaults,
    ToggleTile(String, bool),
    SetOpacity(f32),
    SetOrientation(Orientation),
    SetAccent(String),
    SetFahrenheit(bool),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let settings = AppSettings::load().unwrap_or_default();

        let menu = Menu::new();
        let settings_item = MenuItem::new("Settings", true, None);
        let settings_id = settings_item.id().clone();
        let exit_item = MenuItem::new("Exit", true, None);
        let exit_id = exit_item.id().clone();
        menu.append(&settings_item).expect("tray menu append");
        menu.append(&exit_item).expect("tray menu append");

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("fluidMonitor")
            .with_icon(make_tray_icon())
            .build()
            .expect("tray icon build");

        let widget_size = match settings.orientation {
            Orientation::Vertical => VERTICAL_SIZE,
            Orientation::Horizontal => HORIZONTAL_SIZE,
        };

        let position = if settings.first_run_complete {
            window::Position::Specific(iced::Point::new(settings.window_x as f32, settings.window_y as f32))
        } else {
            window::Position::Centered
        };

        let (_id, open_task) = window::open(window::Settings {
            size: widget_size,
            position,
            decorations: false,
            transparent: true,
            resizable: false,
            level: window::Level::AlwaysOnTop,
            ..Default::default()
        });

        (
            Self {
                settings,
                snapshot: SensorSnapshot::default(),
                poller: None,
                windows: BTreeMap::new(),
                _tray: tray,
                settings_id,
                exit_id,
            },
            open_task.map(|id| Message::WindowOpened(id, WindowKind::Widget)),
        )
    }

    fn widget_window(&self) -> Option<window::Id> {
        self.windows.iter()
            .find(|(_, k)| **k == WindowKind::Widget)
            .map(|(id, _)| *id)
    }

    fn settings_window(&self) -> Option<window::Id> {
        self.windows.iter()
            .find(|(_, k)| **k == WindowKind::Settings)
            .map(|(id, _)| *id)
    }

    fn open_settings(&mut self) -> Task<Message> {
        if self.settings_window().is_some() {
            return Task::none();
        }
        let position = match (self.settings.settings_window_x, self.settings.settings_window_y) {
            (Some(x), Some(y)) => window::Position::Specific(iced::Point::new(x as f32, y as f32)),
            _ => window::Position::Default,
        };
        let (_id, open_task) = window::open(window::Settings {
            size: SETTINGS_SIZE,
            position,
            decorations: false,
            transparent: true,
            resizable: false,
            level: window::Level::AlwaysOnTop,
            ..Default::default()
        });
        open_task.map(|id| Message::WindowOpened(id, WindowKind::Settings))
    }

    fn widget_resize_task(&self) -> Task<Message> {
        self.widget_window().map(|id| {
            let size = match self.settings.orientation {
                Orientation::Vertical => VERTICAL_SIZE,
                Orientation::Horizontal => HORIZONTAL_SIZE,
            };
            window::resize(id, size)
        }).unwrap_or(Task::none())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SensorTick => {
                let poller = self.poller.get_or_insert_with(SensorPoller::new);
                self.snapshot = poller.poll();
                Task::none()
            }
            Message::TrayPoll => {
                if let Ok(event) = MenuEvent::receiver().try_recv() {
                    if event.id == self.exit_id {
                        return iced::exit();
                    }
                    if event.id == self.settings_id {
                        return self.open_settings();
                    }
                }
                Task::none()
            }
            Message::DragWindow(id) => window::drag(id),
            Message::WindowOpened(id, kind) => {
                self.windows.insert(id, kind);
                Task::none()
            }
            Message::WindowMoved(id, pos) => {
                match self.windows.get(&id) {
                    Some(&WindowKind::Widget) => {
                        self.settings.window_x = pos.x as f64;
                        self.settings.window_y = pos.y as f64;
                        self.settings.first_run_complete = true;
                        let _ = self.settings.save();
                    }
                    Some(&WindowKind::Settings) => {
                        self.settings.settings_window_x = Some(pos.x as f64);
                        self.settings.settings_window_y = Some(pos.y as f64);
                        let _ = self.settings.save();
                    }
                    None => {}
                }
                Task::none()
            }
            Message::WindowClosed(id) => {
                self.windows.remove(&id);
                if self.widget_window().is_none() {
                    return iced::exit();
                }
                Task::none()
            }
            Message::SaveClose => {
                let _ = self.settings.save();
                let close = self.settings_window().map(window::close).unwrap_or(Task::none());
                Task::batch([close, self.widget_resize_task()])
            }
            Message::ResetDefaults => {
                self.settings = AppSettings::default();
                Task::none()
            }
            Message::ToggleTile(name, on) => {
                if on {
                    if !self.settings.visible_tiles.contains(&name) {
                        self.settings.visible_tiles.push(name);
                    }
                } else {
                    self.settings.visible_tiles.retain(|t| t != &name);
                }
                Task::none()
            }
            Message::SetOpacity(v) => {
                self.settings.widget_opacity = v;
                Task::none()
            }
            Message::SetOrientation(o) => {
                self.settings.orientation = o;
                self.widget_resize_task()
            }
            Message::SetAccent(hex) => {
                self.settings.theme_accent = hex;
                Task::none()
            }
            Message::SetFahrenheit(f) => {
                self.settings.temperature_unit = if f { TempUnit::Fahrenheit } else { TempUnit::Celsius };
                Task::none()
            }
        }
    }

    fn view(&self, id: window::Id) -> Element<'_, Message> {
        let p = Palette::from_settings(&self.settings);

        match self.windows.get(&id) {
            Some(WindowKind::Settings) => settings_panel::view(&self.settings, p, id),
            _ => self.widget_view(id, p),
        }
    }

    fn widget_view(&self, id: window::Id, p: Palette) -> Element<'_, Message> {
        let mut tiles: Vec<Element<'_, Message>> = Vec::new();
        for name in &self.settings.tile_order {
            if !self.settings.visible_tiles.contains(name) {
                continue;
            }
            let el = match name.as_str() {
                "CPU" => tile::cpu_tile(&self.snapshot.cpu, &self.settings, p),
                "GPU" => tile::gpu_tile(&self.snapshot.gpu, &self.settings, p),
                "RAM" => tile::ram_tile(&self.snapshot.ram, &self.settings, p),
                "Disk" => tile::disk_tile(&self.snapshot.disk, &self.settings, p),
                "Network" => tile::network_tile(&self.snapshot.network, &self.settings, p),
                _ => continue,
            };
            tiles.push(el);
        }

        let body: Element<'_, Message> = match self.settings.orientation {
            Orientation::Vertical => column(tiles).spacing(5).into(),
            Orientation::Horizontal => row(tiles).spacing(5).into(),
        };

        let root = container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(8)
            .style(move |_| iced::widget::container::Style {
                background: Some(iced::Background::Color(p.bg)),
                border: Border { radius: 8.0.into(), ..Border::default() },
                ..Default::default()
            });

        mouse_area(root)
            .on_press(Message::DragWindow(id))
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::time::every(Duration::from_secs(1)).map(|_| Message::SensorTick),
            iced::time::every(Duration::from_millis(200)).map(|_| Message::TrayPoll),
            window::close_events().map(Message::WindowClosed),
            window::events().map(|(id, event)| match event {
                window::Event::Moved(pos) => Message::WindowMoved(id, pos),
                _ => Message::TrayPoll,
            }),
        ])
    }

    fn theme(&self, _id: window::Id) -> Theme {
        Theme::Dark
    }
}

