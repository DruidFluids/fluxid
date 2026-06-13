mod tile;
mod style;

use fluid_core::sensor_data::SensorSnapshot;
use fluid_core::settings::AppSettings;
use fluid_sensor::SensorPoller;
use iced::widget::{column, container, mouse_area};
use iced::{window, Border, Element, Length, Size, Subscription, Task, Theme};
use std::time::Duration;
use style::FluidTheme;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    TrayIcon, TrayIconBuilder,
};

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    iced::application("fluidMonitor", App::update, App::view)
        .subscription(App::subscription)
        .theme(App::theme)
        .window(window::Settings {
            size: Size::new(240.0, 330.0),
            decorations: false,
            transparent: true,
            resizable: false,
            level: window::Level::AlwaysOnTop,
            ..Default::default()
        })
        .run_with(App::new)
}

fn make_tray_icon() -> tray_icon::Icon {
    // 32x32 solid accent-blue square with rounded feel (placeholder until real .ico)
    const SIZE: u32 = 32;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            // simple rounded-corner mask
            let corner = 6i32;
            let xi = x as i32;
            let yi = y as i32;
            let s = SIZE as i32;
            let in_corner = (xi < corner && yi < corner && (corner - xi) * (corner - xi) + (corner - yi) * (corner - yi) > corner * corner)
                || (xi >= s - corner && yi < corner && (xi - (s - corner)) * (xi - (s - corner)) + (corner - yi) * (corner - yi) > corner * corner)
                || (xi < corner && yi >= s - corner && (corner - xi) * (corner - xi) + (yi - (s - corner)) * (yi - (s - corner)) > corner * corner)
                || (xi >= s - corner && yi >= s - corner && (xi - (s - corner)) * (xi - (s - corner)) + (yi - (s - corner)) * (yi - (s - corner)) > corner * corner);
            if in_corner {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                rgba.extend_from_slice(&[77, 153, 255, 255]); // #4D99FF
            }
        }
    }
    tray_icon::Icon::from_rgba(rgba, SIZE, SIZE).expect("icon from rgba")
}

struct App {
    settings: AppSettings,
    snapshot: SensorSnapshot,
    poller: Option<SensorPoller>,
    _tray: TrayIcon,
    exit_id: tray_icon::menu::MenuId,
}

#[derive(Debug, Clone)]
enum Message {
    SensorTick,
    TrayPoll,
    DragWindow,
    Exit,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let settings = AppSettings::load().unwrap_or_default();

        let menu = Menu::new();
        let exit_item = MenuItem::new("Exit", true, None);
        let exit_id = exit_item.id().clone();
        menu.append(&exit_item).expect("tray menu append");

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("fluidMonitor")
            .with_icon(make_tray_icon())
            .build()
            .expect("tray icon build");

        (
            Self {
                settings,
                snapshot: SensorSnapshot::default(),
                poller: None,
                _tray: tray,
                exit_id,
            },
            Task::none(),
        )
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
                }
                Task::none()
            }
            Message::DragWindow => {
                window::get_latest().and_then(window::drag)
            }
            Message::Exit => iced::exit(),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let tiles = column![
            tile::cpu_tile(&self.snapshot.cpu, &self.settings),
            tile::gpu_tile(&self.snapshot.gpu, &self.settings),
            tile::ram_tile(&self.snapshot.ram, &self.settings),
            tile::disk_tile(&self.snapshot.disk, &self.settings),
            tile::network_tile(&self.snapshot.network, &self.settings),
        ]
        .spacing(5);

        let root = container(tiles)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(8)
            .style(|_| iced::widget::container::Style {
                background: Some(iced::Background::Color(FluidTheme::BG)),
                border: Border {
                    radius: 8.0.into(),
                    ..Border::default()
                },
                ..Default::default()
            });

        mouse_area(root)
            .on_press(Message::DragWindow)
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::time::every(Duration::from_secs(1)).map(|_| Message::SensorTick),
            iced::time::every(Duration::from_millis(200)).map(|_| Message::TrayPoll),
        ])
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }
}
